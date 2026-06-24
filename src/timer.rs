use std::fmt;
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::config::{Config, Rgba};

pub static TIMER_STATE: OnceLock<Mutex<TimerState>> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ClockTime {
    hour: u32,
    minute: u32,
}

impl fmt::Display for ClockTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}", self.hour, self.minute)
    }
}

#[derive(Debug)]
pub struct TimerState {
    pub elapsed_secs: u64,
    last_time: Option<ClockTime>,
    pub last_tick: Option<Instant>,
    pub interval: u64,
    pub schedule_reminder: Vec<crate::config::ScheduleReminder>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReminderKind {
    Schedule,
    Interval,
}

#[derive(Debug, PartialEq, Eq)]
struct ReminderEvent {
    kind: ReminderKind,
    bg_color: Rgba,
    time: ClockTime,
}

impl ReminderEvent {
    fn new(kind: ReminderKind, bg_color: Rgba, time: ClockTime) -> Self {
        Self { kind, bg_color, time }
    }

    fn label(&self) -> &'static str {
        match self.kind {
            ReminderKind::Schedule => "Schedule reminder",
            ReminderKind::Interval => "Interval reminder",
        }
    }
}

impl TimerState {
    pub fn new(config: &Config) -> Self {
        TimerState {
            elapsed_secs: 0,
            last_time: None,
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
    let events = {
        let mut state = TIMER_STATE.get().unwrap().lock().unwrap();
        collect_due_reminders(&mut state, config)
    };

    dispatch_reminders(config, events);
}

pub fn reset_timer(config: &Config) {
    let time = {
        let mut state = TIMER_STATE.get().unwrap().lock().unwrap();
        state.elapsed_secs = 0;
        get_current_time()
    };

    dispatch_reminders(
        config,
        vec![ReminderEvent::new(
            ReminderKind::Interval,
            config.interval_reminder.bg_color,
            time,
        )],
    );
}

fn collect_due_reminders(state: &mut TimerState, config: &Config) -> Vec<ReminderEvent> {
    let now = Instant::now();
    let elapsed = elapsed_since_last_tick(state, now);
    state.elapsed_secs += elapsed;
    state.last_tick = Some(now);

    collect_due_reminders_at(state, config, get_current_time())
}

fn collect_due_reminders_at(
    state: &mut TimerState,
    config: &Config,
    current_time: ClockTime,
) -> Vec<ReminderEvent> {
    let mut events = collect_schedule_events(state, current_time);

    if events.is_empty() && state.elapsed_secs >= state.interval {
        state.elapsed_secs = 0;
        events.push(ReminderEvent::new(
            ReminderKind::Interval,
            config.interval_reminder.bg_color,
            current_time,
        ));
    }

    events
}

fn elapsed_since_last_tick(state: &TimerState, now: Instant) -> u64 {
    if let Some(last) = state.last_tick {
        let millis = now.duration_since(last).as_millis() as u64;
        (millis + 500) / 1000
    } else {
        1
    }
}

fn collect_schedule_events(state: &mut TimerState, current_time: ClockTime) -> Vec<ReminderEvent> {
    if state.last_time == Some(current_time) {
        return Vec::new();
    }

    state.last_time = Some(current_time);
    let current_time_str = current_time.to_string();

    state
        .schedule_reminder
        .iter()
        .filter(|reminder| reminder.time == current_time_str)
        .map(|reminder| ReminderEvent::new(ReminderKind::Schedule, reminder.bg_color, current_time))
        .collect()
}

fn dispatch_reminders(config: &Config, events: Vec<ReminderEvent>) {
    for event in events {
        trigger_reminder(config, event);
    }
}

fn trigger_reminder(config: &Config, event: ReminderEvent) {
    let time_str = event.time.to_string();
    log::info!("{} triggered at {}", event.label(), time_str);
    crate::overlay::show_overlay_with_params(crate::overlay::OverlayParams::from_config(
        config,
        event.bg_color,
        time_str,
    ));
}

fn get_current_time() -> ClockTime {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = duration.as_secs();
    let secs_in_day = total_secs % 86400;
    let hours = ((secs_in_day / 3600) + 8) % 24;
    let minutes = (secs_in_day % 3600) / 60;

    ClockTime {
        hour: hours as u32,
        minute: minutes as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{IntervalReminder, ScheduleReminder};

    fn test_config() -> Config {
        Config {
            interval_reminder: IntervalReminder {
                interval: 300,
                bg_color: Rgba(1, 2, 3, 4),
            },
            schedule_reminder: vec![
                ScheduleReminder {
                    time: "09:30".to_string(),
                    bg_color: Rgba(10, 20, 30, 40),
                },
                ScheduleReminder {
                    time: "09:30".to_string(),
                    bg_color: Rgba(50, 60, 70, 80),
                },
            ],
            ..Config::default()
        }
    }

    #[test]
    fn schedule_events_trigger_once_per_minute() {
        let config = test_config();
        let mut state = TimerState::new(&config);
        state.elapsed_secs = 120;

        let first = collect_due_reminders_at(&mut state, &config, ClockTime { hour: 9, minute: 30 });
        let second = collect_due_reminders_at(&mut state, &config, ClockTime { hour: 9, minute: 30 });

        assert_eq!(first.len(), 2);
        assert!(first.iter().all(|event| event.kind == ReminderKind::Schedule));
        assert!(second.is_empty());
    }

    #[test]
    fn schedule_events_take_priority_over_interval() {
        let config = test_config();
        let mut state = TimerState::new(&config);
        state.elapsed_secs = config.interval_reminder.interval;

        let events = collect_due_reminders_at(&mut state, &config, ClockTime { hour: 9, minute: 30 });

        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|event| event.kind == ReminderKind::Schedule));
        assert_eq!(state.elapsed_secs, config.interval_reminder.interval);
    }

    #[test]
    fn interval_event_resets_elapsed_time_when_no_schedule_matches() {
        let config = test_config();
        let mut state = TimerState::new(&config);
        state.elapsed_secs = config.interval_reminder.interval;

        let events = collect_due_reminders_at(&mut state, &config, ClockTime { hour: 9, minute: 31 });

        assert_eq!(
            events,
            vec![ReminderEvent::new(
                ReminderKind::Interval,
                config.interval_reminder.bg_color,
                ClockTime { hour: 9, minute: 31 },
            )]
        );
        assert_eq!(state.elapsed_secs, 0);
    }

    #[test]
    fn elapsed_time_rounds_to_nearest_second() {
        let config = test_config();
        let mut state = TimerState::new(&config);
        let now = Instant::now();

        state.last_tick = Some(now - std::time::Duration::from_millis(1499));
        assert_eq!(elapsed_since_last_tick(&state, now), 1);

        state.last_tick = Some(now - std::time::Duration::from_millis(1500));
        assert_eq!(elapsed_since_last_tick(&state, now), 2);
    }
}
