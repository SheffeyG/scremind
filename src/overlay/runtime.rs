use std::error::Error;
use std::mem;
use std::sync::Once;

use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{CreateSolidBrush, InvalidateRect, UpdateWindow};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::animation::AnimationUpdate;

use super::hooks::{
    clear_active_window, set_active_window, InputHookGuards, WM_OVERLAY_INPUT_RECEIVED,
};
use super::render::paint_overlay;
use super::types::{OverlayParams, OverlayWindowState};

static REGISTER_OVERLAY_CLASS: Once = Once::new();

const OVERLAY_TIMER_ID: usize = 1;

pub struct OverlayRuntime;

impl OverlayRuntime {
    pub fn run(params: OverlayParams) -> Result<(), Box<dyn Error>> {
        unsafe {
            let h_instance = GetModuleHandleW(None)?;
            let h_instance = HINSTANCE(h_instance.0);

            register_overlay_class(h_instance);
            let hwnd = create_overlay_window(h_instance, OverlayWindowState::new(params))?;
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

            message_loop();
            Ok(())
        }
    }
}

unsafe fn register_overlay_class(h_instance: HINSTANCE) {
    REGISTER_OVERLAY_CLASS.call_once(|| {
        let window_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(overlay_wnd_proc),
            hInstance: h_instance,
            lpszClassName: w!("OverlayWindowClass"),
            hbrBackground: CreateSolidBrush(COLORREF(0)),
            ..mem::zeroed()
        };
        let atom = RegisterClassW(&window_class);
        if atom == 0 {
            log::error!("Failed to register overlay window class");
        }
    });
}

unsafe fn create_overlay_window(
    h_instance: HINSTANCE,
    state: OverlayWindowState,
) -> Result<HWND, Box<dyn Error>> {
    let state_ptr = Box::into_raw(Box::new(state));
    let hwnd = CreateWindowExW(
        WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
        w!("OverlayWindowClass"),
        w!("Overlay"),
        WS_POPUP,
        0,
        0,
        GetSystemMetrics(SM_CXSCREEN),
        GetSystemMetrics(SM_CYSCREEN),
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

unsafe fn message_loop() {
    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
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
        WM_TIMER => handle_timer(hwnd),
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

    let timer_interval = state.view.timer_interval_ms;
    if SetTimer(hwnd, OVERLAY_TIMER_ID, timer_interval, None) == 0 {
        log::error!(
            "Failed to start overlay timer: interval={}ms, fps={}",
            timer_interval,
            state.view.fps
        );
        return LRESULT(-1);
    }

    log::info!(
        "Overlay timer started: time_str={}, interval={}ms, fps={}",
        state.view.time_str,
        timer_interval,
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

unsafe fn handle_timer(hwnd: HWND) -> LRESULT {
    let Some(state) = window_state_mut(hwnd) else {
        log::error!("Overlay WM_TIMER missing window state");
        return LRESULT(0);
    };

    match state.runtime.animation.tick(state.runtime.input_received) {
        AnimationUpdate::Continue(alpha) => {
            state.runtime.current_alpha = alpha;
            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
            let _ = InvalidateRect(hwnd, None, false);
        }
        AnimationUpdate::Close => {
            let _ = DestroyWindow(hwnd);
        }
    }

    LRESULT(0)
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
    let _ = KillTimer(hwnd, OVERLAY_TIMER_ID);
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
