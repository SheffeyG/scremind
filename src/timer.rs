use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::config::Config;

pub static TIMER_STATE: OnceLock<Mutex<TimerState>> = OnceLock::new();

#[derive(Debug)]
pub struct TimerState {
    pub elapsed_secs: u64,
    pub last_time: String,
    pub last_tick: Option<Instant>,
    pub interval: u64,
    pub schedule_reminder: Vec<crate::config::ScheduleReminder>,
}

impl TimerState {
    pub fn new(config: &Config) -> Self {
        TimerState {
            elapsed_secs: 0,
            last_time: String::new(),
            last_tick: Some(Instant::now()),
            interval: config.interval_reminder.interval,
            schedule_reminder: config.schedule_reminder.clone(),
        }
    }
}

pub fn init(config: &Config) {
    let state = TimerState::new(config);
    TIMER_STATE.set(Mutex::new(state)).expect("TimerState already initialized");
    log::info!("Timer initialized: interval={}s", config.interval_reminder.interval);
}

pub fn get_remaining_time() -> u64 {
    let state = TIMER_STATE.get().unwrap().lock().unwrap();
    let remaining_secs = state.interval.saturating_sub(state.elapsed_secs);
    (remaining_secs + 59) / 60
}

pub fn get_schedule_reminders() -> Vec<String> {
    let state = TIMER_STATE.get().unwrap().lock().unwrap();
    state.schedule_reminder.iter().map(|r| r.time.clone()).collect()
}

pub fn tick(config: &Config) {
    let mut state = TIMER_STATE.get().unwrap().lock().unwrap();
    let now = Instant::now();

    let elapsed = if let Some(last) = state.last_tick {
        let millis = now.duration_since(last).as_millis() as u64;
        (millis + 500) / 1000
    } else {
        1
    };
    state.elapsed_secs += elapsed;
    state.last_tick = Some(now);

    let scheduled_triggered = check_schedule_reminders(&mut state, config);

    if !scheduled_triggered && state.elapsed_secs >= state.interval {
        state.elapsed_secs = 0;
        let now = get_current_time();
        let time_str = format!("{:02}:{:02}", now.0, now.1);
        log::info!("Interval reminder triggered at {}", time_str);
        crate::overlay::show_overlay_with_params(
            crate::overlay::OverlayParams::from_config(config, config.interval_reminder.bg_color, time_str),
        );
    }
}

pub fn trigger_interval_reminder(config: &Config) {
    let mut state = TIMER_STATE.get().unwrap().lock().unwrap();
    state.elapsed_secs = 0;
    drop(state);

    let now = get_current_time();
    let time_str = format!("{:02}:{:02}", now.0, now.1);
    log::info!("Manual interval reminder triggered at {}", time_str);
    crate::overlay::show_overlay_with_params(
        crate::overlay::OverlayParams::from_config(config, config.interval_reminder.bg_color, time_str),
    );
}

fn check_schedule_reminders(state: &mut TimerState, config: &Config) -> bool {
    let now = get_current_time();
    let current_time = format!("{:02}:{:02}", now.0, now.1);

    if current_time == state.last_time {
        return false;
    }
    state.last_time = current_time.clone();

    let mut triggered = false;
    for reminder in &state.schedule_reminder {
        if reminder.time == current_time {
            log::info!("Schedule reminder triggered: {}", current_time);
            crate::overlay::show_overlay_with_params(
                crate::overlay::OverlayParams::from_config(config, reminder.bg_color, current_time.clone()),
            );
            triggered = true;
        }
    }
    triggered
}

fn get_current_time() -> (u32, u32) {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = duration.as_secs();
    let secs_in_day = total_secs % 86400;
    let hours = ((secs_in_day / 3600) + 8) % 24;
    let minutes = (secs_in_day % 3600) / 60;
    (hours as u32, minutes as u32)
}
