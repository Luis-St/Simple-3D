//! Mouse buttons and the drags bound to them.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

impl MouseButton {
    pub const ALL: [MouseButton; 3] = [MouseButton::Left, MouseButton::Middle, MouseButton::Right];

    pub fn label(self) -> &'static str {
        match self {
            MouseButton::Left => "Left",
            MouseButton::Middle => "Middle",
            MouseButton::Right => "Right",
        }
    }
}

/// A mouse drag binding for a navigation action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Drag {
    pub button: MouseButton,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub alt: bool,
}

impl Drag {
    pub fn new(button: MouseButton) -> Drag {
        Drag { button, ctrl: false, shift: false, alt: false }
    }

    pub fn with_shift(button: MouseButton) -> Drag {
        Drag { shift: true, ..Drag::new(button) }
    }

    pub fn with_ctrl(button: MouseButton) -> Drag {
        Drag { ctrl: true, ..Drag::new(button) }
    }

    /// Modifier state has to match exactly, so `Shift+Middle` for pan does not
    /// also fire plain-`Middle` orbit.
    pub fn matches(&self, button: MouseButton, ctrl: bool, shift: bool, alt: bool) -> bool {
        self.button == button && self.ctrl == ctrl && self.shift == shift && self.alt == alt
    }
}

impl fmt::Display for Drag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ctrl {
            write!(f, "Ctrl+")?;
        }
        if self.alt {
            write!(f, "Alt+")?;
        }
        if self.shift {
            write!(f, "Shift+")?;
        }
        write!(f, "{} drag", self.button.label())
    }
}
