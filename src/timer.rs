use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::Config;

pub static TIMER_STATE: Mutex<TimerState> = Mutex::new(TimerState {
    elapsed_secs: 0,
    last_time: String::new(),
    interval: 0,
    scheduled_reminders: Vec::new(),
    fade_duration: 0.0,
    hold_duration: 0.0,
    fps: 0,
    font_size: 0,
    font_name: String::new(),
    font_color: (0, 0, 0, 0),
});

pub struct TimerState {
    pub elapsed_secs: u64,
    pub last_time: String,
    pub interval: u64,
    pub scheduled_reminders: Vec<crate::config::ScheduledReminder>,
    pub fade_duration: f64,
    pub hold_duration: f64,
    pub fps: u32,
    pub font_size: i32,
    pub font_name: String,
    pub font_color: (u8, u8, u8, u8),
}

impl TimerState {
    pub fn new(config: &Config) -> Self {
        TimerState {
            elapsed_secs: 0,
            last_time: String::new(),
            interval: config.interval_reminder.interval,
            scheduled_reminders: config.scheduled_reminders.clone(),
            fade_duration: config.overlay.fade_duration,
            hold_duration: config.overlay.hold_duration,
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

pub fn init(config: &Config) {
    let mut state = TIMER_STATE.lock().unwrap();
    *state = TimerState::new(config);
}

pub fn get_remaining_time() -> u64 {
    let state = TIMER_STATE.lock().unwrap();
    let remaining_secs = state.interval.saturating_sub(state.elapsed_secs);
    (remaining_secs + 59) / 60
}

pub fn get_scheduled_reminders() -> Vec<String> {
    let state = TIMER_STATE.lock().unwrap();
    state.scheduled_reminders.iter().map(|r| r.time.clone()).collect()
}

pub fn tick(config: &Config) {
    let mut state = TIMER_STATE.lock().unwrap();
    state.elapsed_secs += 1;

    let scheduled_triggered = check_scheduled_reminders(&mut state);

    if !scheduled_triggered && state.elapsed_secs >= state.interval {
        state.elapsed_secs = 0;
        let now = get_current_time();
        let time_str = format!("{:02}:{:02}", now.0, now.1);
        crate::overlay::show_overlay_with_params(crate::overlay::OverlayParams {
            alpha: config.interval_reminder.color.a,
            fade_duration: state.fade_duration,
            hold_duration: state.hold_duration,
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

pub fn trigger_interval_reminder(config: &Config) {
    let mut state = TIMER_STATE.lock().unwrap();
    state.elapsed_secs = 0;
    let fade_duration = state.fade_duration;
    let hold_duration = state.hold_duration;
    let fps = state.fps;
    let font_size = state.font_size;
    let font_name = state.font_name.clone();
    let font_color = state.font_color;
    drop(state);

    let now = get_current_time();
    let time_str = format!("{:02}:{:02}", now.0, now.1);
    crate::overlay::show_overlay_with_params(crate::overlay::OverlayParams {
        alpha: config.interval_reminder.color.a,
        fade_duration,
        hold_duration,
        fps,
        color: (
            config.interval_reminder.color.r,
            config.interval_reminder.color.g,
            config.interval_reminder.color.b,
        ),
        time_str,
        font_size,
        font_name,
        font_color,
    });
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
                hold_duration: state.hold_duration,
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
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = duration.as_secs();
    let secs_in_day = total_secs % 86400;
    let hours = ((secs_in_day / 3600) + 8) % 24;
    let minutes = (secs_in_day % 3600) / 60;
    (hours as u32, minutes as u32)
}
