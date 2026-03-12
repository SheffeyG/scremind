#![windows_subsystem = "windows"]

mod autostart;
mod config;
mod overlay;
mod timer;
mod tray;

use std::mem;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

use config::Config;

static CONFIG: OnceLock<Config> = OnceLock::new();
static RUNNING: AtomicBool = AtomicBool::new(true);

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    unsafe {
        let config = Config::load("config.toml")?;
        CONFIG.set(config.clone()).expect("Config already set");

        timer::init(&config);
        autostart::init();

        let h_instance = GetModuleHandleW(None)?;

        let wnd_class = WNDCLASSW {
            lpfnWndProc: Some(tray::wnd_proc),
            hInstance: h_instance.into(),
            lpszClassName: w!("TrayWindowClass"),
            ..mem::zeroed()
        };

        RegisterClassW(&wnd_class);

        let hwnd = CreateWindowExW(
            WS_EX_NOACTIVATE,
            w!("TrayWindowClass"),
            w!("Screen Reminder"),
            WS_OVERLAPPED,
            0, 0, 0, 0,
            None, None, h_instance, None,
        )?;

        let nid = tray::create_nid(hwnd);
        let _ = Shell_NotifyIconW(NIM_ADD, &nid);

        let timer_id: usize = 1;
        SetTimer(hwnd, timer_id, 1000, None);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);

            if !RUNNING.load(Ordering::SeqCst) {
                break;
            }
        }

        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);

        Ok(())
    }
}
