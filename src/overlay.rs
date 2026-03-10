use std::mem;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

static INPUT_RECEIVED: AtomicBool = AtomicBool::new(false);
static OVERLAY_ACTIVE: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, PartialEq)]
enum FadeState {
    FadeIn,
    Hold,
    FadeOut,
}

struct WindowState {
    start_time: std::time::Instant,
    fade_state: FadeState,
    current_alpha: u8,
    target_alpha: u8,
    fade_duration: f64,
    fps: u32,
    color: (u8, u8, u8),
}

pub struct OverlayParams {
    pub alpha: u8,
    pub fade_duration: f64,
    pub fps: u32,
    pub color: (u8, u8, u8),
}

pub fn show_overlay_with_params(params: OverlayParams) {
    if OVERLAY_ACTIVE.load(Ordering::SeqCst) {
        return;
    }
    OVERLAY_ACTIVE.store(true, Ordering::SeqCst);
    INPUT_RECEIVED.store(false, Ordering::SeqCst);

    std::thread::spawn(move || {
        unsafe {
            if let Err(_) = run_overlay(params.alpha, params.fade_duration, params.fps, params.color) {}
            OVERLAY_ACTIVE.store(false, Ordering::SeqCst);
        }
    });
}

unsafe fn run_overlay(
    target_alpha: u8,
    fade_duration: f64,
    fps: u32,
    color: (u8, u8, u8),
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let h_instance = GetModuleHandleW(None)?;

    let window_class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(overlay_wnd_proc),
        hInstance: h_instance.into(),
        lpszClassName: w!("OverlayWindowClass"),
        hbrBackground: CreateSolidBrush(COLORREF(0)),
        ..mem::zeroed()
    };

    RegisterClassW(&window_class);

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
        None,
    )?;

    let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 0, LWA_ALPHA);

    let keyboard_hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), h_instance, 0)?;
    let mouse_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), h_instance, 0)?;

    let state = Box::new(WindowState {
        start_time: std::time::Instant::now(),
        fade_state: FadeState::FadeIn,
        current_alpha: 0,
        target_alpha,
        fade_duration,
        fps,
        color,
    });
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as _);

    let _ = ShowWindow(hwnd, SW_SHOW);
    let _ = UpdateWindow(hwnd);

    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }

    let _ = UnhookWindowsHookEx(keyboard_hook);
    let _ = UnhookWindowsHookEx(mouse_hook);

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

unsafe extern "system" fn overlay_wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);

            let rect = &mut RECT::default();
            let _ = GetClientRect(hwnd, rect);

            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            let (r, g, b) = if !state_ptr.is_null() {
                let state = &*state_ptr;
                state.color
            } else {
                (255, 255, 255)
            };

            let color = COLORREF((r as u32) | ((g as u32) << 8) | ((b as u32) << 16));
            let brush = CreateSolidBrush(color);
            FillRect(hdc, rect, brush);
            let _ = DeleteObject(brush);

            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
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
        WM_TIMER => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !state_ptr.is_null() {
                let state = &mut *state_ptr;
                let elapsed = state.start_time.elapsed().as_secs_f64();
                let target_alpha = state.target_alpha;
                let fade_duration = state.fade_duration;

                match state.fade_state {
                    FadeState::FadeIn => {
                        let progress = elapsed / fade_duration;
                        if progress >= 1.0 {
                            state.current_alpha = target_alpha;
                            state.fade_state = FadeState::Hold;
                        } else {
                            state.current_alpha = (target_alpha as f64 * progress) as u8;
                        }
                    }
                    FadeState::Hold => {
                        state.current_alpha = target_alpha;
                        if INPUT_RECEIVED.load(Ordering::SeqCst) {
                            state.fade_state = FadeState::FadeOut;
                            state.start_time = std::time::Instant::now();
                        }
                    }
                    FadeState::FadeOut => {
                        let progress = elapsed / fade_duration;
                        if progress >= 1.0 {
                            let _ = DestroyWindow(hwnd);
                            return LRESULT(0);
                        } else {
                            state.current_alpha = (target_alpha as f64 * (1.0 - progress)) as u8;
                        }
                    }
                }

                let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), state.current_alpha, LWA_ALPHA);
                let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !state_ptr.is_null() {
                drop(Box::from_raw(state_ptr));
            }
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
