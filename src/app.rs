use simplelog::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::autostart;
use crate::config::Config;
use crate::overlay;
use crate::reminder::ReminderEvent;
use crate::timer::TimerState;
use crate::tray;

static APP_STATE: OnceLock<AppState> = OnceLock::new();

#[derive(Debug)]
struct AppState {
    config: Config,
    timer: Mutex<TimerState>,
    running: AtomicBool,
}

impl AppState {
    fn new(config: Config) -> Self {
        Self {
            timer: Mutex::new(TimerState::new(&config)),
            config,
            running: AtomicBool::new(true),
        }
    }

    fn config(&self) -> &Config {
        &self.config
    }

    fn timer(&self) -> &Mutex<TimerState> {
        &self.timer
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

pub fn get_remaining_time() -> u64 {
    app_state().timer().lock().unwrap().remaining_time()
}

pub fn get_schedule_reminders() -> Vec<String> {
    app_state().timer().lock().unwrap().schedule_reminders()
}

pub fn tick_timer() -> Vec<ReminderEvent> {
    let state = app_state();
    state.timer().lock().unwrap().tick(state.config())
}

pub fn reset_timer() {
    let state = app_state();
    let mut timer = state.timer().lock().unwrap();
    timer.reset_interval();
    timer.request_interval_reminder();
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

pub fn run() -> std::result::Result<(), Box<dyn std::error::Error>> {
    init_logger();

    unsafe {
        let app = App::init()?;
        app.run();
        app.cleanup();
    }

    Ok(())
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
        app_state()
            .timer()
            .lock()
            .unwrap()
            .request_interval_reminder();
        log::info!(
            "Config loaded: interval={}s, schedule_count={}",
            config.interval_reminder.interval,
            config.schedule_reminder.len()
        );

        autostart::init();
        log::info!("Timer and autostart initialized");

        let hwnd = tray::create_window()?;
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
