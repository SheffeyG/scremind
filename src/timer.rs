use crate::config::{Config, ScheduledReminder};
use crate::overlay::OverlayParams;
use std::time::SystemTime;

pub struct Timer {
    elapsed_secs: u64,
    last_time: String,
    interval: u64,
    scheduled_reminders: Vec<ScheduledReminder>,
    fade_duration: f64,
    fps: u32,
}

impl Timer {
    pub fn new(config: &Config) -> Self {
        Timer {
            elapsed_secs: 0,
            last_time: String::new(),
            interval: config.interval_reminder.interval,
            scheduled_reminders: config.scheduled_reminders.clone(),
            fade_duration: config.overlay.fade_duration,
            fps: config.overlay.fps,
        }
    }

    pub fn tick(&mut self, config: &Config) {
        self.elapsed_secs += 1;

        let scheduled_triggered = self.check_scheduled_reminders();

        // Scheduled reminder has priority over interval reminder
        if !scheduled_triggered && self.elapsed_secs >= self.interval {
            self.elapsed_secs = 0;
            println!("Triggering interval reminder...");
            crate::overlay::show_overlay_with_params(OverlayParams {
                alpha: config.interval_reminder.color.a,
                fade_duration: self.fade_duration,
                fps: self.fps,
                color: (
                    config.interval_reminder.color.r,
                    config.interval_reminder.color.g,
                    config.interval_reminder.color.b,
                ),
            });
        }
    }

    fn check_scheduled_reminders(&mut self) -> bool {
        let now = get_current_time();
        let current_time = format!("{:02}:{:02}", now.0, now.1);

        if current_time == self.last_time {
            return false;
        }
        self.last_time = current_time.clone();

        let mut triggered = false;
        for reminder in &self.scheduled_reminders {
            if reminder.time == current_time {
                println!("Triggering scheduled reminder at {}...", reminder.time);
                crate::overlay::show_overlay_with_params(OverlayParams {
                    alpha: reminder.color.a,
                    fade_duration: self.fade_duration,
                    fps: self.fps,
                    color: (reminder.color.r, reminder.color.g, reminder.color.b),
                });
                triggered = true;
            }
        }
        triggered
    }
}

fn get_current_time() -> (u32, u32) {
    use std::time::UNIX_EPOCH;
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = duration.as_secs();
    let secs_in_day = total_secs % 86400;
    let hours = ((secs_in_day / 3600) + 8) % 24;
    let minutes = (secs_in_day % 3600) / 60;
    (hours as u32, minutes as u32)
}
