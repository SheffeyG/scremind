use std::fmt;
use std::time::{Duration, Instant};

use windows::Win32::System::SystemInformation::GetLocalTime;

use crate::config::{Config, Rgba, ScheduleReminder};
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
    interval_bg_color: Rgba,
    schedule_reminder: Vec<ScheduleReminder>,
}

impl TimerState {
    pub fn new(config: &Config) -> Self {
        let interval = Duration::from_secs(config.interval_reminder.interval);
        TimerState {
            next_interval_at: Instant::now() + interval,
            pending_interval_reminder: true,
            last_time: None,
            interval,
            interval_bg_color: config.interval_reminder.bg_color,
            schedule_reminder: config.schedule_reminder.clone(),
        }
    }

    pub fn reset_interval(&mut self) {
        self.next_interval_at = Instant::now() + self.interval;
        self.pending_interval_reminder = true;
    }

    pub fn remaining_time(&self) -> u64 {
        let remaining_secs = self
            .next_interval_at
            .saturating_duration_since(Instant::now())
            .as_secs();

        remaining_secs.div_ceil(60)
    }

    pub fn schedule_reminders(&self) -> Vec<String> {
        self.schedule_reminder
            .iter()
            .map(|r| r.time.clone())
            .collect()
    }

    pub fn tick(&mut self) -> Option<ReminderEvent> {
        self.tick_at(current_time())
    }

    fn tick_at(&mut self, current_time: ClockTime) -> Option<ReminderEvent> {
        self.collect_schedule_event(current_time)
            .or_else(|| self.collect_interval_event(current_time))
    }

    fn collect_schedule_event(&mut self, current_time: ClockTime) -> Option<ReminderEvent> {
        if self.last_time == Some(current_time) {
            return None;
        }

        self.last_time = Some(current_time);
        let current_time_str = current_time.to_string();

        self.schedule_reminder
            .iter()
            .find(|reminder| reminder.time == current_time_str)
            .map(|reminder| {
                ReminderEvent::new(
                    ReminderKind::Schedule,
                    reminder.bg_color,
                    current_time.to_string(),
                )
            })
    }

    fn collect_interval_event(&mut self, current_time: ClockTime) -> Option<ReminderEvent> {
        if self.pending_interval_reminder {
            self.pending_interval_reminder = false;
            return Some(self.interval_reminder_at(current_time));
        }

        let now = Instant::now();
        if now < self.next_interval_at {
            return None;
        }

        if self.interval.is_zero() {
            self.next_interval_at = now;
        } else {
            while self.next_interval_at <= now {
                self.next_interval_at += self.interval;
            }
        }
        Some(self.interval_reminder_at(current_time))
    }

    fn interval_reminder_at(&self, current_time: ClockTime) -> ReminderEvent {
        ReminderEvent::new(
            ReminderKind::Interval,
            self.interval_bg_color,
            current_time.to_string(),
        )
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

        let first = state.collect_schedule_event(ClockTime {
            hour: 9,
            minute: 30,
        });
        let second = state.collect_schedule_event(ClockTime {
            hour: 9,
            minute: 30,
        });

        assert!(matches!(
            first,
            Some(ReminderEvent {
                kind: ReminderKind::Schedule,
                ..
            })
        ));
        assert!(second.is_none());
    }

    #[test]
    fn schedule_events_take_priority_over_interval() {
        let config = test_config();
        let mut state = TimerState::new(&config);
        let now = Instant::now();
        state.next_interval_at = now;

        let event = state.tick_at(ClockTime {
            hour: 9,
            minute: 30,
        });

        assert!(matches!(
            event,
            Some(ReminderEvent {
                kind: ReminderKind::Schedule,
                ..
            })
        ));
        assert_eq!(state.next_interval_at, now);
    }

    #[test]
    fn tick_returns_none_when_no_reminder_is_due() {
        let config = test_config();
        let mut state = TimerState::new(&config);
        state.pending_interval_reminder = false;

        let event = state.tick_at(ClockTime {
            hour: 9,
            minute: 31,
        });

        assert!(event.is_none());
    }

    #[test]
    fn pending_interval_reminder_triggers_once_without_changing_deadline() {
        let config = test_config();
        let mut state = TimerState::new(&config);
        let deadline = state.next_interval_at;

        let event = state.collect_interval_event(ClockTime {
            hour: 9,
            minute: 31,
        });

        assert_eq!(
            event,
            Some(ReminderEvent::new(
                ReminderKind::Interval,
                config.interval_reminder.bg_color,
                "09:31".to_string(),
            ))
        );
        assert_eq!(state.next_interval_at, deadline);
        assert!(!state.pending_interval_reminder);
    }

    #[test]
    fn pending_interval_reminder_preserves_schedule_priority() {
        let config = test_config();
        let mut state = TimerState::new(&config);
        let deadline = state.next_interval_at;

        let event = state.tick_at(ClockTime {
            hour: 9,
            minute: 30,
        });

        assert!(matches!(
            event,
            Some(ReminderEvent {
                kind: ReminderKind::Schedule,
                bg_color,
                ..
            }) if bg_color == config.schedule_reminder[0].bg_color
        ));
        assert_eq!(state.next_interval_at, deadline);
        assert!(state.pending_interval_reminder);
    }

    #[test]
    fn reset_reschedules_interval_and_queues_immediate_reminder() {
        let config = test_config();
        let mut state = TimerState::new(&config);
        state.pending_interval_reminder = false;
        let reset_started_at = Instant::now();

        state.reset_interval();

        assert!(state.next_interval_at >= reset_started_at + state.interval);
        assert!(state.pending_interval_reminder);
    }

    #[test]
    fn interval_event_reschedules_next_interval_when_no_schedule_matches() {
        let config = test_config();
        let mut state = TimerState::new(&config);
        state.pending_interval_reminder = false;
        let now = Instant::now();
        state.next_interval_at = now;

        let event = state.collect_interval_event(ClockTime {
            hour: 9,
            minute: 31,
        });

        assert_eq!(
            event,
            Some(ReminderEvent::new(
                ReminderKind::Interval,
                config.interval_reminder.bg_color,
                "09:31".to_string(),
            ))
        );
        assert_eq!(state.next_interval_at, now + state.interval);
    }

    #[test]
    fn delayed_interval_event_preserves_original_cadence() {
        let config = test_config();
        let mut state = TimerState::new(&config);
        state.pending_interval_reminder = false;
        let original_deadline = Instant::now() - Duration::from_secs(1);
        state.next_interval_at = original_deadline;

        let event = state.collect_interval_event(ClockTime {
            hour: 9,
            minute: 31,
        });

        assert!(event.is_some());
        assert_eq!(state.next_interval_at, original_deadline + state.interval);
    }

    #[test]
    fn delayed_interval_event_skips_missed_deadlines() {
        let config = test_config();
        let mut state = TimerState::new(&config);
        state.pending_interval_reminder = false;
        let original_deadline = Instant::now() - Duration::from_secs(601);
        state.next_interval_at = original_deadline;

        let event = state.collect_interval_event(ClockTime {
            hour: 9,
            minute: 31,
        });

        assert!(event.is_some());
        assert_eq!(
            state.next_interval_at,
            original_deadline + state.interval + state.interval + state.interval
        );
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
