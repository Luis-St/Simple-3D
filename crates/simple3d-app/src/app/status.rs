//! The footer message, and how it fades once read.

use std::time::Duration;

/// A status bar message and its tone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Idle,
    Info(String),
    Warning(String),
}

impl Status {
    pub fn text(&self) -> &str {
        match self {
            Status::Idle => "Ready",
            Status::Info(text) | Status::Warning(text) => text,
        }
    }
}

/// How long a message stays at full strength: readable twice, gone before it goes stale.
pub const STATUS_LIFETIME: Duration = Duration::from_secs(6);

/// Current readability: 1, falling to 0 over the second after its lifetime. `Idle` never fades.
pub fn status_opacity(status: &Status, age: Duration) -> f32 {
    if matches!(status, Status::Idle) {
        return 1.0;
    }
    let over = age.as_secs_f32() - STATUS_LIFETIME.as_secs_f32();
    if over <= 0.0 {
        1.0
    } else {
        (1.0 - over).clamp(0.0, 1.0)
    }
}
