use std::error::Error;
use std::mem;
use std::sync::OnceLock;
use std::thread::sleep;
use std::time::{Duration, Instant};

use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{InvalidateRect, UpdateWindow};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::animation::AnimationUpdate;

use super::hooks::{
    clear_active_window, set_active_window, InputHookGuards, WM_OVERLAY_INPUT_RECEIVED,
};
use super::render::paint_overlay;
use super::types::{OverlayParams, OverlayWindowState};

static OVERLAY_CLASS_REGISTERED: OnceLock<()> = OnceLock::new();

pub struct OverlayRuntime;

impl OverlayRuntime {
    pub fn run(params: OverlayParams) -> Result<(), Box<dyn Error>> {
        unsafe {
            let h_instance = GetModuleHandleW(None)?;
            let h_instance = HINSTANCE(h_instance.0);

            register_overlay_class(h_instance)?;
            let screen_width = GetSystemMetrics(SM_CXSCREEN);
            let screen_height = GetSystemMetrics(SM_CYSCREEN);
            let hwnd = create_overlay_window(
                h_instance,
                OverlayWindowState::new(params, screen_width, screen_height),
                screen_width,
                screen_height,
            )?;
            set_active_window(hwnd);

            let _input_hooks = match InputHookGuards::install(h_instance) {
                Ok(hooks) => hooks,
                Err(e) => {
                    log::error!("Failed to install overlay input hooks: {}", e);
                    let _ = DestroyWindow(hwnd);
                    return Err(Box::new(e));
                }
            };

            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = UpdateWindow(hwnd);

            message_loop(hwnd);
            Ok(())
        }
    }
}

unsafe fn register_overlay_class(h_instance: HINSTANCE) -> windows::core::Result<()> {
    if OVERLAY_CLASS_REGISTERED.get().is_some() {
        return Ok(());
    }

    let window_class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(overlay_wnd_proc),
        hInstance: h_instance,
        lpszClassName: w!("OverlayWindowClass"),
        ..mem::zeroed()
    };

    if RegisterClassW(&window_class) == 0 {
        Err(windows::core::Error::from_win32())
    } else {
        let _ = OVERLAY_CLASS_REGISTERED.set(());
        Ok(())
    }
}

unsafe fn create_overlay_window(
    h_instance: HINSTANCE,
    state: OverlayWindowState,
    screen_width: i32,
    screen_height: i32,
) -> Result<HWND, Box<dyn Error>> {
    let state_ptr = Box::into_raw(Box::new(state));
    let hwnd = CreateWindowExW(
        WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
        w!("OverlayWindowClass"),
        w!("Overlay"),
        WS_POPUP,
        0,
        0,
        screen_width,
        screen_height,
        None,
        None,
        h_instance,
        Some(state_ptr as _),
    );

    match hwnd {
        Ok(hwnd) => {
            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 0, LWA_ALPHA);
            Ok(hwnd)
        }
        Err(e) => {
            log::error!("Failed to create overlay window: {}", e);
            drop(Box::from_raw(state_ptr));
            Err(Box::new(e))
        }
    }
}

unsafe fn message_loop(hwnd: HWND) {
    let Some(state) = window_state_mut(hwnd) else {
        log::error!("Overlay message loop missing window state");
        return;
    };

    let frame_interval = Duration::from_millis(state.view.timer_interval_ms as u64);
    let mut next_frame_at = Instant::now();
    let mut msg = MSG::default();

    loop {
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).into() {
            if msg.message == WM_QUIT {
                return;
            }

            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        if !IsWindow(hwnd).as_bool() {
            return;
        }

        let now = Instant::now();
        if now >= next_frame_at {
            update_animation(hwnd);
            next_frame_at = now + frame_interval;
            continue;
        }

        let sleep_for = (next_frame_at - now).min(Duration::from_millis(1));
        sleep(sleep_for);
    }
}

unsafe extern "system" fn overlay_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCCREATE => handle_nccreate(hwnd, lparam),
        WM_CREATE => handle_create(hwnd),
        WM_PAINT => handle_paint(hwnd),
        WM_ERASEBKGND => LRESULT(1),
        WM_OVERLAY_INPUT_RECEIVED => handle_input_received(hwnd),
        WM_DESTROY => handle_destroy(hwnd),
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn handle_nccreate(hwnd: HWND, lparam: LPARAM) -> LRESULT {
    let create_struct = lparam.0 as *const CREATESTRUCTW;
    if create_struct.is_null() {
        log::error!("Overlay WM_NCCREATE received null CREATESTRUCTW");
        return LRESULT(0);
    }

    let state_ptr = (*create_struct).lpCreateParams as *mut OverlayWindowState;
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as _);
    LRESULT(1)
}

unsafe fn handle_create(hwnd: HWND) -> LRESULT {
    let Some(state) = window_state_mut(hwnd) else {
        log::error!("Overlay WM_CREATE missing window state");
        return LRESULT(-1);
    };

    if let Err(e) = super::render::initialize_renderer(hwnd, state) {
        log::error!("Failed to initialize overlay renderer: {}", e);
        return LRESULT(-1);
    }

    let frame_interval = state.view.timer_interval_ms;
    log::info!(
        "Overlay frame loop started: time_str={}, interval={}ms, fps={}",
        state.view.time_str,
        frame_interval,
        state.view.fps
    );

    LRESULT(0)
}

unsafe fn handle_paint(hwnd: HWND) -> LRESULT {
    let Some(state) = window_state_mut(hwnd) else {
        log::error!("Overlay WM_PAINT missing window state");
        return LRESULT(0);
    };

    paint_overlay(hwnd, state);
    LRESULT(0)
}

unsafe fn update_animation(hwnd: HWND) {
    let Some(state) = window_state_mut(hwnd) else {
        log::error!("Overlay frame update missing window state");
        return;
    };

    match state.runtime.animation.tick(state.runtime.input_received) {
        AnimationUpdate::Continue(alpha) => {
            if alpha != state.runtime.current_alpha {
                state.runtime.current_alpha = alpha;
                let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
                let _ = InvalidateRect(hwnd, None, false);
            }
        }
        AnimationUpdate::Close => {
            let _ = DestroyWindow(hwnd);
        }
    }
}

unsafe fn handle_input_received(hwnd: HWND) -> LRESULT {
    let Some(state) = window_state_mut(hwnd) else {
        log::error!("Overlay input message missing window state");
        return LRESULT(0);
    };

    state.runtime.input_received = true;
    LRESULT(0)
}

unsafe fn handle_destroy(hwnd: HWND) -> LRESULT {
    clear_active_window(hwnd);

    if let Some(state_ptr) = take_window_state(hwnd) {
        drop(Box::from_raw(state_ptr));
    }

    PostQuitMessage(0);
    LRESULT(0)
}

unsafe fn window_state_mut(hwnd: HWND) -> Option<&'static mut OverlayWindowState> {
    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut OverlayWindowState;
    state_ptr.as_mut()
}

unsafe fn take_window_state(hwnd: HWND) -> Option<*mut OverlayWindowState> {
    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut OverlayWindowState;
    if state_ptr.is_null() {
        None
    } else {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        Some(state_ptr)
    }
}
