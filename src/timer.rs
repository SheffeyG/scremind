use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::config::{Config, Rgba};

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

    let current_time = {
        let t = get_current_time();
        format!("{:02}:{:02}", t.0, t.1)
    };

    let mut scheduled_bg_colors: Vec<Rgba> = Vec::new();
    if current_time != state.last_time {
        state.last_time = current_time.clone();
        for reminder in &state.schedule_reminder {
            if reminder.time == current_time {
                scheduled_bg_colors.push(reminder.bg_color);
            }
        }
    }

    let should_trigger_interval = scheduled_bg_colors.is_empty() && state.elapsed_secs >= state.interval;
    if should_trigger_interval {
        state.elapsed_secs = 0;
    }
    drop(state);

    for bg_color in &scheduled_bg_colors {
        trigger_reminder(config, *bg_color, "Schedule reminder");
    }

    if should_trigger_interval {
        trigger_reminder(config, config.interval_reminder.bg_color, "Interval reminder");
    }
}

pub fn reset_timer(config: &Config) {
    {
        let mut state = TIMER_STATE.get().unwrap().lock().unwrap();
        state.elapsed_secs = 0;
    }
    trigger_reminder(config, config.interval_reminder.bg_color, "Interval reminder");
}

pub fn trigger_reminder(config: &Config, bg_color: Rgba, label: &str) {
    let now = get_current_time();
    let time_str = format!("{:02}:{:02}", now.0, now.1);
    log::info!("{} triggered at {}", label, time_str);
    crate::overlay::show_overlay_with_params(
        crate::overlay::OverlayParams::from_config(config, bg_color, time_str),
    );
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
