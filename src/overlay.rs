use std::mem;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;
use windows::core::w;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::animation::{AnimationUpdate, FadeAnimation};
use crate::config::{Config, Rgba};

static INPUT_RECEIVED: AtomicBool = AtomicBool::new(false);
static OVERLAY_ACTIVE: AtomicBool = AtomicBool::new(false);
static REGISTER_OVERLAY_CLASS: Once = Once::new();

struct WindowState {
    animation: FadeAnimation,
    current_alpha: u8,
    fps: u32,
    bg_color: Rgba,
    time_str: String,
    font_size: i32,
    font_name: String,
    fg_color: Rgba,
}

struct OverlayActivationGuard;

impl OverlayActivationGuard {
    fn try_acquire() -> Option<Self> {
        OVERLAY_ACTIVE
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| Self)
    }
}

impl Drop for OverlayActivationGuard {
    fn drop(&mut self) {
        OVERLAY_ACTIVE.store(false, Ordering::SeqCst);
    }
}

struct HookGuard(HHOOK);

impl HookGuard {
    fn install(
        id: WINDOWS_HOOK_ID,
        proc: HOOKPROC,
        h_instance: HINSTANCE,
    ) -> windows::core::Result<Self> {
        Ok(Self(unsafe { SetWindowsHookExW(id, proc, h_instance, 0)? }))
    }
}

impl Drop for HookGuard {
    fn drop(&mut self) {
        let _ = unsafe { UnhookWindowsHookEx(self.0) };
    }
}

pub struct OverlayParams {
    pub bg_color: Rgba,
    pub fade_duration: f64,
    pub hold_duration: [f64; 2],
    pub fps: u32,
    pub time_str: String,
    pub font_size: i32,
    pub font_name: String,
    pub fg_color: Rgba,
}

impl OverlayParams {
    pub fn from_config(config: &Config, bg_color: Rgba, time_str: String) -> Self {
        OverlayParams {
            bg_color,
            fade_duration: config.overlay.fade_duration,
            hold_duration: config.overlay.hold_duration,
            fps: config.overlay.fps,
            time_str,
            font_size: config.foreground.font_size,
            font_name: config.foreground.font_name.clone(),
            fg_color: config.foreground.fg_color,
        }
    }
}

pub fn show_overlay_with_params(params: OverlayParams) {
    let Some(activation_guard) = OverlayActivationGuard::try_acquire() else {
        log::warn!("Overlay already active, skipping");
        return;
    };
    INPUT_RECEIVED.store(false, Ordering::SeqCst);
    log::info!("Showing overlay: time_str={}", params.time_str);

    std::thread::spawn(move || {
        let _activation_guard = activation_guard;
        unsafe {
            if let Err(e) = run_overlay(&params) {
                log::error!("Overlay error: {}", e);
            }
            log::info!("Overlay closed");
        }
    });
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
        RegisterClassW(&window_class);
    });
}

unsafe fn create_overlay_window(
    h_instance: HINSTANCE,
    state: Box<WindowState>,
) -> std::result::Result<HWND, Box<dyn std::error::Error>> {
    let state_ptr = Box::into_raw(state);
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
    let hwnd = match hwnd {
        Ok(hwnd) => hwnd,
        Err(e) => {
            drop(Box::from_raw(state_ptr));
            return Err(Box::new(e));
        }
    };
    let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 0, LWA_ALPHA);
    Ok(hwnd)
}

unsafe fn run_overlay(
    params: &OverlayParams,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let h_instance = GetModuleHandleW(None)?;
    let h_instance = HINSTANCE(h_instance.0);

    register_overlay_class(h_instance);
    let state = Box::new(WindowState {
        animation: FadeAnimation::new(
            params.bg_color.3,
            params.fade_duration,
            params.hold_duration,
        ),
        current_alpha: 0,
        fps: params.fps,
        bg_color: params.bg_color,
        time_str: params.time_str.clone(),
        font_size: params.font_size,
        font_name: params.font_name.clone(),
        fg_color: params.fg_color,
    });
    let hwnd = create_overlay_window(h_instance, state)?;

    let _keyboard_hook = HookGuard::install(WH_KEYBOARD_LL, Some(keyboard_hook_proc), h_instance)?;
    let _mouse_hook = HookGuard::install(WH_MOUSE_LL, Some(mouse_hook_proc), h_instance)?;

    let _ = ShowWindow(hwnd, SW_SHOW);
    let _ = UpdateWindow(hwnd);

    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }

    Ok(())
}

unsafe extern "system" fn keyboard_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        INPUT_RECEIVED.store(true, Ordering::SeqCst);
    }
    CallNextHookEx(None, code, wparam, lparam)
}

unsafe extern "system" fn mouse_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        INPUT_RECEIVED.store(true, Ordering::SeqCst);
    }
    CallNextHookEx(None, code, wparam, lparam)
}

unsafe fn paint_overlay(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);

    let rect = &mut RECT::default();
    let _ = GetClientRect(hwnd, rect);
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;

    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
    let (r, g, b, time_str, font_size, font_name, fg_color) = if !state_ptr.is_null() {
        let state = &*state_ptr;
        (
            state.bg_color.0,
            state.bg_color.1,
            state.bg_color.2,
            state.time_str.as_str(),
            state.font_size,
            state.font_name.as_str(),
            state.fg_color,
        )
    } else {
        (255, 255, 255, "", 72, "Arial", Rgba(255, 255, 255, 255))
    };

    // Double buffering: draw to memory DC first, then BitBlt to screen to avoid flickering
    let mem_dc = CreateCompatibleDC(hdc);
    let mem_bitmap = CreateCompatibleBitmap(hdc, width, height);
    let old_bitmap = SelectObject(mem_dc, mem_bitmap);

    let color = COLORREF((r as u32) | ((g as u32) << 8) | ((b as u32) << 16));
    let brush = CreateSolidBrush(color);
    let mem_rect = RECT {
        left: 0,
        top: 0,
        right: width,
        bottom: height,
    };
    FillRect(mem_dc, &mem_rect, brush);
    let _ = DeleteObject(brush);

    let mut time_wide: Vec<u16> = time_str.encode_utf16().collect();
    let font_name_wide: Vec<u16> = font_name.encode_utf16().chain(std::iter::once(0)).collect();

    let font = CreateFontW(
        font_size,
        0,
        0,
        0,
        FW_NORMAL.0 as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET.0 as u32,
        OUT_DEFAULT_PRECIS.0 as u32,
        CLIP_DEFAULT_PRECIS.0 as u32,
        DEFAULT_QUALITY.0 as u32,
        (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
        windows::core::PCWSTR(font_name_wide.as_ptr()),
    );

    let text_dc = CreateCompatibleDC(hdc);
    let text_bitmap = CreateCompatibleBitmap(hdc, width, height);
    let old_text_bitmap = SelectObject(text_dc, text_bitmap);

    let old_font = SelectObject(text_dc, font);

    let black_brush = CreateSolidBrush(COLORREF(0));
    FillRect(text_dc, &mem_rect, black_brush);
    let _ = DeleteObject(black_brush);

    let text_color =
        COLORREF((fg_color.0 as u32) | ((fg_color.1 as u32) << 8) | ((fg_color.2 as u32) << 16));
    SetTextColor(text_dc, text_color);
    SetBkMode(text_dc, TRANSPARENT);

    let mut text_rect = mem_rect;
    DrawTextW(
        text_dc,
        &mut time_wide,
        &mut text_rect,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );

    let alpha = fg_color.3;
    let blend_fn = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: alpha,
        AlphaFormat: 0,
    };
    let _ = AlphaBlend(
        mem_dc, 0, 0, width, height, text_dc, 0, 0, width, height, blend_fn,
    );

    SelectObject(text_dc, old_font);
    let _ = DeleteObject(font);

    SelectObject(text_dc, old_text_bitmap);
    let _ = DeleteObject(text_bitmap);
    let _ = DeleteDC(text_dc);

    let _ = BitBlt(hdc, 0, 0, width, height, mem_dc, 0, 0, SRCCOPY);

    SelectObject(mem_dc, old_bitmap);
    let _ = DeleteObject(mem_bitmap);
    let _ = DeleteDC(mem_dc);

    let _ = EndPaint(hwnd, &ps);
}

unsafe fn update_fade_state(hwnd: HWND) -> LRESULT {
    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
    if !state_ptr.is_null() {
        let state = &mut *state_ptr;
        let input_received = INPUT_RECEIVED.load(Ordering::SeqCst);

        match state.animation.tick(input_received) {
            AnimationUpdate::Continue(alpha) => {
                state.current_alpha = alpha;
                let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
                let _ = InvalidateRect(hwnd, None, false);
            }
            AnimationUpdate::Close => {
                let _ = DestroyWindow(hwnd);
            }
        }
    }
    LRESULT(0)
}

unsafe extern "system" fn overlay_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCCREATE => {
            let create_struct = lparam.0 as *const CREATESTRUCTW;
            if !create_struct.is_null() {
                let state_ptr = (*create_struct).lpCreateParams as *mut WindowState;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as _);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_PAINT => {
            paint_overlay(hwnd);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_CREATE => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            let timer_interval = if !state_ptr.is_null() {
                let state = &*state_ptr;
                1000u32 / state.fps.max(1)
            } else {
                16
            };
            SetTimer(hwnd, 1, timer_interval, None);
            LRESULT(0)
        }
        WM_TIMER => update_fade_state(hwnd),
        WM_DESTROY => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !state_ptr.is_null() {
                drop(Box::from_raw(state_ptr));
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
