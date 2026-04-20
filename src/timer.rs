use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::config::Config;

pub static TIMER_STATE: Mutex<TimerState> = Mutex::new(TimerState {
    elapsed_secs: 0,
    last_time: String::new(),
    last_tick: None,
    interval: 0,
    schedule_reminder: Vec::new(),
    fade_duration: 0.0,
    hold_duration: [0.0, 0.0],
    fps: 0,
    font_size: 0,
    font_name: String::new(),
    fg_color: (0, 0, 0, 0),
});

pub struct TimerState {
    pub elapsed_secs: u64,
    pub last_time: String,
    pub last_tick: Option<Instant>,
    pub interval: u64,
    pub schedule_reminder: Vec<crate::config::ScheduleReminder>,
    pub fade_duration: f64,
    pub hold_duration: [f64; 2],
    pub fps: u32,
    pub font_size: i32,
    pub font_name: String,
    pub fg_color: (u8, u8, u8, u8),
}

impl TimerState {
    pub fn new(config: &Config) -> Self {
        TimerState {
            elapsed_secs: 0,
            last_time: String::new(),
            last_tick: Some(Instant::now()),
            interval: config.interval_reminder.interval,
            schedule_reminder: config.schedule_reminder.clone(),
            fade_duration: config.overlay.fade_duration,
            hold_duration: config.overlay.hold_duration,
            fps: config.overlay.fps,
            font_size: config.foreground.font_size,
            font_name: config.foreground.font_name.clone(),
            fg_color: (
                config.foreground.fg_color[0],
                config.foreground.fg_color[1],
                config.foreground.fg_color[2],
                config.foreground.fg_color[3],
            ),
        }
    }
}

pub fn init(config: &Config) {
    let mut state = TIMER_STATE.lock().unwrap();
    *state = TimerState::new(config);
    log::info!("Timer initialized: interval={}s", config.interval_reminder.interval);
}

pub fn get_remaining_time() -> u64 {
    let state = TIMER_STATE.lock().unwrap();
    let remaining_secs = state.interval.saturating_sub(state.elapsed_secs);
    (remaining_secs + 59) / 60
}

pub fn get_schedule_reminders() -> Vec<String> {
    let state = TIMER_STATE.lock().unwrap();
    state.schedule_reminder.iter().map(|r| r.time.clone()).collect()
}

pub fn tick(config: &Config) {
    let mut state = TIMER_STATE.lock().unwrap();
    let now = Instant::now();

    let elapsed = if let Some(last) = state.last_tick {
        now.duration_since(last).as_secs()
    } else {
        1
    };
    state.elapsed_secs += elapsed;
    state.last_tick = Some(now);

    let scheduled_triggered = check_schedule_reminders(&mut state);

    if !scheduled_triggered && state.elapsed_secs >= state.interval {
        state.elapsed_secs = 0;
        let now = get_current_time();
        let time_str = format!("{:02}:{:02}", now.0, now.1);
        log::info!("Interval reminder triggered at {}", time_str);
        crate::overlay::show_overlay_with_params(crate::overlay::OverlayParams {
            alpha: config.interval_reminder.bg_color[3],
            fade_duration: state.fade_duration,
            hold_duration: state.hold_duration,
            fps: state.fps,
            color: (
                config.interval_reminder.bg_color[0],
                config.interval_reminder.bg_color[1],
                config.interval_reminder.bg_color[2],
            ),
            time_str,
            font_size: state.font_size,
            font_name: state.font_name.clone(),
            fg_color: state.fg_color,
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
    let fg_color = state.fg_color;
    drop(state);

    let now = get_current_time();
    let time_str = format!("{:02}:{:02}", now.0, now.1);
    log::info!("Manual interval reminder triggered at {}", time_str);
    crate::overlay::show_overlay_with_params(crate::overlay::OverlayParams {
        alpha: config.interval_reminder.bg_color[3],
        fade_duration,
        hold_duration,
        fps,
        color: (
            config.interval_reminder.bg_color[0],
            config.interval_reminder.bg_color[1],
            config.interval_reminder.bg_color[2],
        ),
        time_str,
        font_size,
        font_name,
        fg_color,
    });
}

fn check_schedule_reminders(state: &mut TimerState) -> bool {
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
            crate::overlay::show_overlay_with_params(crate::overlay::OverlayParams {
                alpha: reminder.bg_color[3],
                fade_duration: state.fade_duration,
                hold_duration: state.hold_duration,
                fps: state.fps,
                color: (reminder.bg_color[0], reminder.bg_color[1], reminder.bg_color[2]),
                time_str: current_time.clone(),
                font_size: state.font_size,
                font_name: state.font_name.clone(),
                fg_color: state.fg_color,
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
