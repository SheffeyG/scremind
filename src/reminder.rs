use crate::config::Rgba;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReminderKind {
    Schedule,
    Interval,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ReminderEvent {
    pub kind: ReminderKind,
    pub bg_color: Rgba,
    pub time: String,
}

impl ReminderEvent {
    pub fn new(kind: ReminderKind, bg_color: Rgba, time: String) -> Self {
        Self {
            kind,
            bg_color,
            time,
        }
    }

    pub fn label(&self) -> &'static str {
        match self.kind {
            ReminderKind::Schedule => "Schedule reminder",
            ReminderKind::Interval => "Interval reminder",
        }
    }
}
