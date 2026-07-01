#![windows_subsystem = "windows"]

mod animation;
mod autostart;
mod config;
mod overlay;
mod reminder;
mod timer;
mod tray;

use std::mem;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use windows::core::w;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use config::Config;
use reminder::ReminderEvent;
use simplelog::*;

static APP_STATE: OnceLock<AppState> = OnceLock::new();

#[derive(Debug)]
struct AppState {
    config: Config,
    running: AtomicBool,
}

impl AppState {
    fn new(config: Config) -> Self {
        Self {
            config,
            running: AtomicBool::new(true),
        }
    }

    fn config(&self) -> &Config {
        &self.config
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

fn app_state() -> &'static AppState {
    APP_STATE.get().expect("AppState not initialized")
}

pub fn config() -> &'static Config {
    app_state().config()
}

pub fn shutdown(hwnd: HWND) {
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &tray::create_nid(hwnd));
        let _ = KillTimer(hwnd, 1);
        app_state().stop();
        let _ = DestroyWindow(hwnd);
    }
}

pub fn dispatch_reminders(events: Vec<ReminderEvent>) {
    for event in events {
        log::info!("{} triggered at {}", event.label(), event.time);
        overlay::show_overlay_with_params(overlay::OverlayParams::from_config(
            config(),
            event.bg_color,
            event.time,
        ));
    }
}

fn init_logger() {
    let exe_path = std::env::current_exe().unwrap_or_default();
    let log_path = exe_path.with_extension("log");

    let log_file = match std::fs::File::create(&log_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to create log file: {}", e);
            return;
        }
    };

    let mut config_builder = ConfigBuilder::new();
    let _ = config_builder.set_time_offset_to_local();
    let config = config_builder.build();

    let _ = WriteLogger::init(LevelFilter::Info, config, log_file);
    log::info!("Logger initialized, log file: {}", log_path.display());
}

struct App {
    hwnd: HWND,
    nid: NOTIFYICONDATAW,
}

impl App {
    unsafe fn init() -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let config = Config::load("config.toml")?;
        APP_STATE
            .set(AppState::new(config.clone()))
            .expect("AppState already set");
        log::info!(
            "Config loaded: interval={}s, schedule_count={}",
            config.interval_reminder.interval,
            config.schedule_reminder.len()
        );

        timer::init(&config);
        autostart::init();
        log::info!("Timer and autostart initialized");

        let hwnd = create_tray_window()?;
        let nid = tray::create_nid(hwnd);
        let _ = Shell_NotifyIconW(NIM_ADD, &nid);
        SetTimer(hwnd, 1, 1000, None);
        log::info!("Tray icon created, timer started");

        Ok(Self { hwnd, nid })
    }

    unsafe fn run(&self) {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);

            if !app_state().is_running() {
                break;
            }
        }
    }

    unsafe fn cleanup(&self) {
        let _ = Shell_NotifyIconW(NIM_DELETE, &self.nid);
        let _ = KillTimer(self.hwnd, 1);
        log::info!("Application exiting");
    }
}

unsafe fn create_tray_window() -> std::result::Result<HWND, Box<dyn std::error::Error>> {
    let h_instance = GetModuleHandleW(None)?;

    let wnd_class = WNDCLASSW {
        lpfnWndProc: Some(tray::wnd_proc),
        hInstance: h_instance.into(),
        lpszClassName: w!("TrayWindowClass"),
        ..mem::zeroed()
    };

    RegisterClassW(&wnd_class);

    Ok(CreateWindowExW(
        WS_EX_NOACTIVATE,
        w!("TrayWindowClass"),
        w!("Screen Reminder"),
        WS_OVERLAPPED,
        0,
        0,
        0,
        0,
        None,
        None,
        h_instance,
        None,
    )?)
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    init_logger();

    unsafe {
        let app = App::init()?;
        app.run();
        app.cleanup();
    }

    Ok(())
}
