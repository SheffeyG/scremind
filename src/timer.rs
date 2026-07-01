use std::fmt;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use windows::Win32::System::SystemInformation::GetLocalTime;

use crate::config::{Config, ScheduleReminder};
use crate::reminder::{ReminderEvent, ReminderKind};

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
    next_interval_at: Instant,
    last_time: Option<ClockTime>,
    interval: Duration,
    schedule_reminder: Vec<ScheduleReminder>,
}

impl TimerState {
    pub fn new(config: &Config) -> Self {
        let interval = Duration::from_secs(config.interval_reminder.interval);
        TimerState {
            next_interval_at: Instant::now() + interval,
            last_time: None,
            interval,
            schedule_reminder: config.schedule_reminder.clone(),
        }
    }

    fn reset_interval(&mut self, now: Instant) {
        self.next_interval_at = now + self.interval;
    }
}

pub fn init(config: &Config) {
    let state = TimerState::new(config);
    TIMER_STATE
        .set(Mutex::new(state))
        .expect("TimerState already initialized");
    log::info!(
        "Timer initialized: interval={}s",
        config.interval_reminder.interval
    );
}

pub fn get_remaining_time() -> u64 {
    let state = TIMER_STATE.get().unwrap().lock().unwrap();
    remaining_minutes_until(state.next_interval_at, Instant::now())
}

pub fn get_schedule_reminders() -> Vec<String> {
    let state = TIMER_STATE.get().unwrap().lock().unwrap();
    state
        .schedule_reminder
        .iter()
        .map(|r| r.time.clone())
        .collect()
}

pub fn tick(config: &Config) -> Vec<ReminderEvent> {
    let mut state = TIMER_STATE.get().unwrap().lock().unwrap();
    collect_due_reminders(&mut state, config, Instant::now(), get_current_time())
}

pub fn reset_timer(config: &Config) -> Vec<ReminderEvent> {
    let time = {
        let mut state = TIMER_STATE.get().unwrap().lock().unwrap();
        state.reset_interval(Instant::now());
        get_current_time()
    };

    vec![ReminderEvent::new(
        ReminderKind::Interval,
        config.interval_reminder.bg_color,
        time.to_string(),
    )]
}

fn collect_due_reminders(
    state: &mut TimerState,
    config: &Config,
    now: Instant,
    current_time: ClockTime,
) -> Vec<ReminderEvent> {
    let events = collect_schedule_events(state, current_time);
    if !events.is_empty() {
        return events;
    }

    if now >= state.next_interval_at {
        state.reset_interval(now);
        return vec![ReminderEvent::new(
            ReminderKind::Interval,
            config.interval_reminder.bg_color,
            current_time.to_string(),
        )];
    }

    Vec::new()
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
        .map(|reminder| {
            ReminderEvent::new(
                ReminderKind::Schedule,
                reminder.bg_color,
                current_time.to_string(),
            )
        })
        .collect()
}

fn remaining_minutes_until(deadline: Instant, now: Instant) -> u64 {
    let remaining_secs = deadline.saturating_duration_since(now).as_secs();
    (remaining_secs + 59) / 60
}

fn get_current_time() -> ClockTime {
    unsafe {
        let time = GetLocalTime();

        ClockTime {
            hour: time.wHour as u32,
            minute: time.wMinute as u32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{IntervalReminder, Rgba, ScheduleReminder};

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
        let now = Instant::now();

        let first = collect_due_reminders(
            &mut state,
            &config,
            now,
            ClockTime {
                hour: 9,
                minute: 30,
            },
        );
        let second = collect_due_reminders(
            &mut state,
            &config,
            now,
            ClockTime {
                hour: 9,
                minute: 30,
            },
        );

        assert_eq!(first.len(), 2);
        assert!(first
            .iter()
            .all(|event| event.kind == ReminderKind::Schedule));
        assert!(second.is_empty());
    }

    #[test]
    fn schedule_events_take_priority_over_interval() {
        let config = test_config();
        let mut state = TimerState::new(&config);
        let now = Instant::now();
        state.next_interval_at = now;

        let events = collect_due_reminders(
            &mut state,
            &config,
            now,
            ClockTime {
                hour: 9,
                minute: 30,
            },
        );

        assert_eq!(events.len(), 2);
        assert!(events
            .iter()
            .all(|event| event.kind == ReminderKind::Schedule));
        assert_eq!(state.next_interval_at, now);
    }

    #[test]
    fn interval_event_reschedules_next_interval_when_no_schedule_matches() {
        let config = test_config();
        let mut state = TimerState::new(&config);
        let now = Instant::now();
        state.next_interval_at = now;

        let events = collect_due_reminders(
            &mut state,
            &config,
            now,
            ClockTime {
                hour: 9,
                minute: 31,
            },
        );

        assert_eq!(
            events,
            vec![ReminderEvent::new(
                ReminderKind::Interval,
                config.interval_reminder.bg_color,
                "09:31".to_string(),
            )]
        );
        assert_eq!(state.next_interval_at, now + state.interval);
    }

    #[test]
    fn remaining_time_rounds_up_to_minutes() {
        let now = Instant::now();

        assert_eq!(remaining_minutes_until(now, now), 0);
        assert_eq!(
            remaining_minutes_until(now + Duration::from_secs(1), now),
            1
        );
        assert_eq!(
            remaining_minutes_until(now + Duration::from_secs(60), now),
            1
        );
        assert_eq!(
            remaining_minutes_until(now + Duration::from_secs(61), now),
            2
        );
    }
}
