use std::fmt;
use std::time::{Duration, Instant};

use windows::Win32::System::SystemInformation::GetLocalTime;

use crate::config::{Config, ScheduleReminder};
use crate::reminder::{ReminderEvent, ReminderKind};

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
    pending_interval_reminder: bool,
    last_time: Option<ClockTime>,
    interval: Duration,
    schedule_reminder: Vec<ScheduleReminder>,
}

impl TimerState {
    pub fn new(config: &Config) -> Self {
        let interval = Duration::from_secs(config.interval_reminder.interval);
        TimerState {
            next_interval_at: Instant::now() + interval,
            pending_interval_reminder: false,
            last_time: None,
            interval,
            schedule_reminder: config.schedule_reminder.clone(),
        }
    }

    pub fn reset_interval(&mut self) {
        self.next_interval_at = Instant::now() + self.interval;
    }

    pub fn request_interval_reminder(&mut self) {
        self.pending_interval_reminder = true;
    }

    pub fn remaining_time(&self) -> u64 {
        let remaining_secs = self
            .next_interval_at
            .saturating_duration_since(Instant::now())
            .as_secs();

        (remaining_secs + 59) / 60
    }

    pub fn schedule_reminders(&self) -> Vec<String> {
        self.schedule_reminder
            .iter()
            .map(|r| r.time.clone())
            .collect()
    }

    pub fn tick(&mut self, config: &Config) -> Vec<ReminderEvent> {
        let current_time = current_time();

        let schedule_events = self.collect_schedule_reminder_events(current_time);
        if !schedule_events.is_empty() {
            return schedule_events;
        }

        if let Some(event) = self.collect_interval_reminder_event(config, current_time) {
            return vec![event];
        }

        Vec::new()
    }

    fn collect_schedule_reminder_events(&mut self, current_time: ClockTime) -> Vec<ReminderEvent> {
        if self.last_time == Some(current_time) {
            return Vec::new();
        }

        self.last_time = Some(current_time);
        let current_time_str = current_time.to_string();

        self.schedule_reminder
            .iter()
            .find(|reminder| reminder.time == current_time_str)
            .map(|reminder| {
                vec![ReminderEvent::new(
                    ReminderKind::Schedule,
                    reminder.bg_color,
                    current_time.to_string(),
                )]
            })
            .unwrap_or_default()
    }

    fn collect_interval_reminder_event(
        &mut self,
        config: &Config,
        current_time: ClockTime,
    ) -> Option<ReminderEvent> {
        if self.pending_interval_reminder {
            self.pending_interval_reminder = false;
            return Some(ReminderEvent::new(
                ReminderKind::Interval,
                config.interval_reminder.bg_color,
                current_time.to_string(),
            ));
        }

        let now = Instant::now();
        if now < self.next_interval_at {
            return None;
        }

        self.next_interval_at = now + self.interval;
        Some(ReminderEvent::new(
            ReminderKind::Interval,
            config.interval_reminder.bg_color,
            current_time.to_string(),
        ))
    }
}

fn current_time() -> ClockTime {
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
    fn schedule_event_triggers_once_per_minute() {
        let config = test_config();
        let mut state = TimerState::new(&config);

        let first = state.collect_schedule_reminder_events(ClockTime {
            hour: 9,
            minute: 30,
        });
        let second = state.collect_schedule_reminder_events(ClockTime {
            hour: 9,
            minute: 30,
        });

        assert_eq!(first.len(), 1);
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

        let events = state.collect_schedule_reminder_events(ClockTime {
            hour: 9,
            minute: 30,
        });

        assert_eq!(events.len(), 1);
        assert!(events
            .iter()
            .all(|event| event.kind == ReminderKind::Schedule));
        assert_eq!(state.next_interval_at, now);
    }

    #[test]
    fn pending_interval_reminder_triggers_once_before_normal_interval() {
        let config = test_config();
        let mut state = TimerState::new(&config);
        state.request_interval_reminder();

        let events = state
            .collect_interval_reminder_event(
                &config,
                ClockTime {
                    hour: 9,
                    minute: 31,
                },
            )
            .into_iter()
            .collect::<Vec<_>>();

        assert_eq!(
            events,
            vec![ReminderEvent::new(
                ReminderKind::Interval,
                config.interval_reminder.bg_color,
                "09:31".to_string(),
            )]
        );
        assert!(!state.pending_interval_reminder);
    }

    #[test]
    fn interval_event_reschedules_next_interval_when_no_schedule_matches() {
        let config = test_config();
        let mut state = TimerState::new(&config);
        let now = Instant::now();
        state.next_interval_at = now;

        let events = state
            .collect_interval_reminder_event(
                &config,
                ClockTime {
                    hour: 9,
                    minute: 31,
                },
            )
            .into_iter()
            .collect::<Vec<_>>();

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
        let config = test_config();
        let mut state = TimerState::new(&config);
        let now = Instant::now();

        state.next_interval_at = now;
        assert_eq!(state.remaining_time(), 0);

        state.next_interval_at = Instant::now() + Duration::from_secs(1);
        assert_eq!(state.remaining_time(), 1);

        state.next_interval_at = Instant::now() + Duration::from_secs(60);
        assert_eq!(state.remaining_time(), 1);

        state.next_interval_at = Instant::now() + Duration::from_secs(61);
        assert_eq!(state.remaining_time(), 2);
    }
}
