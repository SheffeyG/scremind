#![windows_subsystem = "windows"]

mod config;
mod overlay;

use std::mem;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

use config::Config;

static CONFIG: OnceLock<Config> = OnceLock::new();
static RUNNING: AtomicBool = AtomicBool::new(true);
static TIMER_STATE: OnceLock<Mutex<TimerState>> = OnceLock::new();

const WM_TRAYICON: u32 = WM_USER + 1;
const ID_TRAY_EXIT: u32 = 1001;
const ID_TRAY_RESET: u32 = 1002;

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    unsafe {
        let config = Config::load("config.toml")?;
        CONFIG.set(config.clone()).expect("Config already set");

        TIMER_STATE.get_or_init(|| Mutex::new(TimerState::new(&config)));

        let h_instance = GetModuleHandleW(None)?;

        let wnd_class = WNDCLASSW {
            lpfnWndProc: Some(tray_wnd_proc),
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

        let nid = create_nid(hwnd);
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

unsafe extern "system" fn tray_wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_TRAYICON => {
            if lparam.0 as u32 == WM_RBUTTONUP {
                let mut pt = POINT { x: 0, y: 0 };
                let _ = GetCursorPos(&mut pt);

                let h_menu = CreatePopupMenu().ok();
                if let Some(h_menu) = h_menu {
                    let remaining = get_remaining_time();
                    let next_text = format!("Next break in {} mins\0", remaining);
                    let next_text_wide: Vec<u16> = next_text.encode_utf16().collect();
                    let _ = AppendMenuW(
                        h_menu,
                        MF_STRING | MF_DISABLED,
                        0,
                        windows::core::PCWSTR(next_text_wide.as_ptr())
                    );

                    let scheduled = get_scheduled_reminders();
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
                        RUNNING.store(false, Ordering::SeqCst);
                        let _ = PostQuitMessage(0);
                    } else if cmd.0 == ID_TRAY_RESET as i32 {
                        trigger_interval_reminder();
                    }
                }
            }
            LRESULT(0)
        }
        WM_TIMER => {
            let cfg = CONFIG.get().unwrap();
            tick_timer(cfg);
            LRESULT(0)
        }
        WM_DESTROY => {
            RUNNING.store(false, Ordering::SeqCst);
            let _ = PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn get_remaining_time() -> u64 {
    let state = TIMER_STATE.get().unwrap().lock().unwrap();
    let remaining_secs = state.interval.saturating_sub(state.elapsed_secs);
    (remaining_secs + 59) / 60
}

fn get_scheduled_reminders() -> Vec<String> {
    let state = TIMER_STATE.get().unwrap().lock().unwrap();
    state.scheduled_reminders.iter().map(|r| r.time.clone()).collect()
}

fn trigger_interval_reminder() {
    let mut state = TIMER_STATE.get().unwrap().lock().unwrap();
    state.elapsed_secs = 0;
    let fade_duration = state.fade_duration;
    let fps = state.fps;
    let font_size = state.font_size;
    let font_name = state.font_name.clone();
    let font_color = state.font_color;
    drop(state);

    let cfg = CONFIG.get().unwrap();
    let now = get_current_time();
    let time_str = format!("{:02}:{:02}", now.0, now.1);
    crate::overlay::show_overlay_with_params(crate::overlay::OverlayParams {
        alpha: cfg.interval_reminder.color.a,
        fade_duration,
        fps,
        color: (
            cfg.interval_reminder.color.r,
            cfg.interval_reminder.color.g,
            cfg.interval_reminder.color.b,
        ),
        time_str,
        font_size,
        font_name,
        font_color,
    });
}

unsafe fn create_nid(hwnd: HWND) -> NOTIFYICONDATAW {
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

struct TimerState {
    elapsed_secs: u64,
    last_time: String,
    interval: u64,
    scheduled_reminders: Vec<config::ScheduledReminder>,
    fade_duration: f64,
    fps: u32,
    font_size: i32,
    font_name: String,
    font_color: (u8, u8, u8, u8),
}

impl TimerState {
    fn new(config: &Config) -> Self {
        TimerState {
            elapsed_secs: 0,
            last_time: String::new(),
            interval: config.interval_reminder.interval,
            scheduled_reminders: config.scheduled_reminders.clone(),
            fade_duration: config.overlay.fade_duration,
            fps: config.overlay.fps,
            font_size: config.foreground.font_size,
            font_name: config.foreground.font_name.clone(),
            font_color: (
                config.foreground.font_color.r,
                config.foreground.font_color.g,
                config.foreground.font_color.b,
                config.foreground.font_color.a,
            ),
        }
    }
}

fn tick_timer(config: &Config) {
    let mut state = TIMER_STATE.get().unwrap().lock().unwrap();
    state.elapsed_secs += 1;

    let scheduled_triggered = check_scheduled_reminders(&mut state);

    if !scheduled_triggered && state.elapsed_secs >= state.interval {
        state.elapsed_secs = 0;
        let now = get_current_time();
        let time_str = format!("{:02}:{:02}", now.0, now.1);
        crate::overlay::show_overlay_with_params(crate::overlay::OverlayParams {
            alpha: config.interval_reminder.color.a,
            fade_duration: state.fade_duration,
            fps: state.fps,
            color: (
                config.interval_reminder.color.r,
                config.interval_reminder.color.g,
                config.interval_reminder.color.b,
            ),
            time_str,
            font_size: state.font_size,
            font_name: state.font_name.clone(),
            font_color: state.font_color,
        });
    }
}

fn check_scheduled_reminders(state: &mut TimerState) -> bool {
    let now = get_current_time();
    let current_time = format!("{:02}:{:02}", now.0, now.1);

    if current_time == state.last_time {
        return false;
    }
    state.last_time = current_time.clone();

    let mut triggered = false;
    for reminder in &state.scheduled_reminders {
        if reminder.time == current_time {
            crate::overlay::show_overlay_with_params(crate::overlay::OverlayParams {
                alpha: reminder.color.a,
                fade_duration: state.fade_duration,
                fps: state.fps,
                color: (reminder.color.r, reminder.color.g, reminder.color.b),
                time_str: current_time.clone(),
                font_size: state.font_size,
                font_name: state.font_name.clone(),
                font_color: state.font_color,
            });
            triggered = true;
        }
    }
    triggered
}

fn get_current_time() -> (u32, u32) {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = duration.as_secs();
    let secs_in_day = total_secs % 86400;
    let hours = ((secs_in_day / 3600) + 8) % 24;
    let minutes = (secs_in_day % 3600) / 60;
    (hours as u32, minutes as u32)
}
