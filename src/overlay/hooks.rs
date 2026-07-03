use std::ffi::c_void;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicPtr, Ordering};

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, PostMessageW, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, HOOKPROC,
    WH_KEYBOARD_LL, WH_MOUSE_LL, WINDOWS_HOOK_ID,
};

pub const WM_OVERLAY_INPUT_RECEIVED: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 1;

static ACTIVE_OVERLAY_HWND: AtomicPtr<c_void> = AtomicPtr::new(null_mut());

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

pub struct InputHookGuards {
    _keyboard: HookGuard,
    _mouse: HookGuard,
}

impl InputHookGuards {
    pub fn install(h_instance: HINSTANCE) -> windows::core::Result<Self> {
        Ok(Self {
            _keyboard: HookGuard::install(WH_KEYBOARD_LL, Some(keyboard_hook_proc), h_instance)?,
            _mouse: HookGuard::install(WH_MOUSE_LL, Some(mouse_hook_proc), h_instance)?,
        })
    }
}

pub fn set_active_window(hwnd: HWND) {
    ACTIVE_OVERLAY_HWND.store(hwnd.0, Ordering::SeqCst);
}

pub fn clear_active_window(hwnd: HWND) {
    let _ = ACTIVE_OVERLAY_HWND.compare_exchange(
        hwnd.0,
        null_mut(),
        Ordering::SeqCst,
        Ordering::SeqCst,
    );
}

fn post_input_received() {
    let hwnd_raw = ACTIVE_OVERLAY_HWND.load(Ordering::SeqCst);
    if !hwnd_raw.is_null() {
        let hwnd = HWND(hwnd_raw);
        let _ = unsafe { PostMessageW(hwnd, WM_OVERLAY_INPUT_RECEIVED, WPARAM(0), LPARAM(0)) };
    }
}

unsafe extern "system" fn keyboard_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        post_input_received();
    }
    CallNextHookEx(None, code, wparam, lparam)
}

unsafe extern "system" fn mouse_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        post_input_received();
    }
    CallNextHookEx(None, code, wparam, lparam)
}
