//! The message in the footer, and how it fades once it has been read.

use std::time::Duration;

/// A message for the status bar, and how it should read.
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

/// How long a message stays at full strength before fading out. Long enough to
/// read twice, short enough that the bar is not still reporting an export that
/// finished ten minutes ago.
pub const STATUS_LIFETIME: Duration = Duration::from_secs(6);

/// How readable a message is now: 1 while it is current, falling to 0 over the
/// second after its lifetime. `Idle` never fades -- "Ready" is a state, not news.
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
