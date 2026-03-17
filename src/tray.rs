use std::mem;
use std::sync::atomic::Ordering;
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

use crate::autostart;
use crate::timer;

const WM_TRAYICON: u32 = WM_USER + 1;
const ID_TRAY_EXIT: u32 = 1001;
const ID_TRAY_RESET: u32 = 1002;
const ID_TRAY_AUTOSTART: u32 = 1003;

pub unsafe fn create_nid(hwnd: HWND) -> NOTIFYICONDATAW {
    let h_instance = GetModuleHandleW(None).unwrap_or_default();

    let h_icon: HICON = LoadImageW(
        h_instance,
        windows::core::PCWSTR(101 as *const u16),
        IMAGE_ICON,
        GetSystemMetrics(SM_CXSMICON),
        GetSystemMetrics(SM_CYSMICON),
        LR_DEFAULTSIZE | LR_SHARED,
    )
    .ok()
    .map(|h| HICON(h.0))
    .unwrap_or_else(|| LoadIconW(None, IDC_WAIT).unwrap_or_default());

    let mut nid = NOTIFYICONDATAW {
        cbSize: mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
        uCallbackMessage: WM_TRAYICON,
        hIcon: h_icon,
        ..mem::zeroed()
    };
    let tip: Vec<u16> = "Screen Reminder\0".encode_utf16().collect();
    nid.szTip[..tip.len()].copy_from_slice(&tip);
    nid
}

pub unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_TRAYICON => {
            if lparam.0 as u32 == WM_RBUTTONUP {
                show_menu(hwnd);
            }
            LRESULT(0)
        }
        WM_TIMER => {
            let cfg = crate::CONFIG.get().unwrap();
            timer::tick(cfg);
            LRESULT(0)
        }
        WM_DESTROY => {
            crate::RUNNING.store(false, Ordering::SeqCst);
            let _ = PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn show_menu(hwnd: HWND) {
    let mut pt = POINT { x: 0, y: 0 };
    let _ = GetCursorPos(&mut pt);

    let h_menu = CreatePopupMenu().ok();
    if let Some(h_menu) = h_menu {
        let remaining = timer::get_remaining_time();
        let next_text = format!("Next break in {} mins\0", remaining);
        let next_text_wide: Vec<u16> = next_text.encode_utf16().collect();
        let _ = AppendMenuW(
            h_menu,
            MF_STRING | MF_DISABLED,
            0,
            windows::core::PCWSTR(next_text_wide.as_ptr())
        );

        let scheduled = timer::get_schedule_reminders();
        if !scheduled.is_empty() {
            let _ = AppendMenuW(h_menu, MF_SEPARATOR, 0, w!(""));
            for reminder in &scheduled {
                let text = format!("Scheduled: {}\0", reminder);
                let text_wide: Vec<u16> = text.encode_utf16().collect();
                let _ = AppendMenuW(
                    h_menu,
                    MF_STRING | MF_DISABLED,
                    0,
                    windows::core::PCWSTR(text_wide.as_ptr())
                );
            }
        }

        let _ = AppendMenuW(h_menu, MF_SEPARATOR, 0, w!(""));

        let autostart_checked = if autostart::is_enabled() { MF_CHECKED } else { MENU_ITEM_FLAGS(0) };
        let _ = AppendMenuW(h_menu, MF_STRING | autostart_checked, ID_TRAY_AUTOSTART as usize, w!("Auto start"));

        let _ = AppendMenuW(h_menu, MF_STRING, ID_TRAY_RESET as usize, w!("Reset"));
        let _ = AppendMenuW(h_menu, MF_STRING, ID_TRAY_EXIT as usize, w!("Exit"));

        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(
            h_menu,
            TPM_RIGHTALIGN | TPM_BOTTOMALIGN | TPM_RETURNCMD,
            pt.x,
            pt.y,
            0,
            hwnd,
            None,
        );
        let _ = DestroyMenu(h_menu);

        if cmd.0 == ID_TRAY_EXIT as i32 {
            let _ = Shell_NotifyIconW(NIM_DELETE, &create_nid(hwnd));
            crate::RUNNING.store(false, Ordering::SeqCst);
            let _ = PostQuitMessage(0);
        } else if cmd.0 == ID_TRAY_RESET as i32 {
            let cfg = crate::CONFIG.get().unwrap();
            timer::trigger_interval_reminder(cfg);
        } else if cmd.0 == ID_TRAY_AUTOSTART as i32 {
            autostart::toggle();
        }
    }
}
