mod config;
mod overlay;
mod timer;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Duration;
use windows::Win32::Foundation::*;
use windows::Win32::System::Console::*;

use config::Config;
use timer::Timer;

static RUNNING: AtomicBool = AtomicBool::new(true);
static CONFIG: OnceLock<Config> = OnceLock::new();

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let config = Config::load("config.toml")?;
    let fade_duration = config.overlay.fade_duration;
    let fps = config.overlay.fps;
    let interval = config.interval_reminder.interval;
    let scheduled_reminders = config.scheduled_reminders.clone();

    CONFIG.set(config.clone()).expect("Config already set");

    unsafe {
        let _ = SetConsoleCtrlHandler(Some(console_ctrl_handler), true);
    }

    println!("Screen overlay reminder started. Press Ctrl+C to exit.");
    println!("Overlay settings: fade_duration={}s, fps={}", fade_duration, fps);

    let interval_cfg = CONFIG.get().unwrap();
    println!(
        "Interval reminder: every {} minutes (RGBA: {}, {}, {}, {})",
        interval / 60,
        interval_cfg.interval_reminder.color.r,
        interval_cfg.interval_reminder.color.g,
        interval_cfg.interval_reminder.color.b,
        interval_cfg.interval_reminder.color.a
    );

    if !scheduled_reminders.is_empty() {
        println!("Scheduled reminders:");
        for reminder in &scheduled_reminders {
            println!(
                "  - {} (RGBA: {}, {}, {}, {})",
                reminder.time, reminder.color.r, reminder.color.g, reminder.color.b, reminder.color.a
            );
        }
    }

    let mut timer = Timer::new(&config);

    while RUNNING.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_secs(1));

        if !RUNNING.load(Ordering::SeqCst) {
            break;
        }

        let cfg = CONFIG.get().unwrap();
        timer.tick(cfg);
    }

    println!("Exiting...");
    Ok(())
}

unsafe extern "system" fn console_ctrl_handler(ctrl_type: u32) -> BOOL {
    if ctrl_type == CTRL_C_EVENT || ctrl_type == CTRL_CLOSE_EVENT {
        RUNNING.store(false, Ordering::SeqCst);
        BOOL(1)
    } else {
        BOOL(0)
    }
}
