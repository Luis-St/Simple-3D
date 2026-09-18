//! Where a model opened while the application is already running goes.

use serde::{Deserialize, Serialize};

/// What opening a model does when there is already a document open (issue 107):
/// give it a tab in the window it was opened from, or a window of its own.
///
/// A tab by default, because that is what the application has always done and
/// what keeps one model's window from becoming five. The other answer is for the
/// work a tab cannot do: two models side by side on one screen, or one per
/// display.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenTarget {
    /// A tab in the window the file was opened from.
    #[default]
    Tab,
    /// A window of its own.
    Window,
}

impl OpenTarget {
    pub const ALL: [OpenTarget; 2] = [OpenTarget::Tab, OpenTarget::Window];

    pub fn label(self) -> &'static str {
        match self {
            OpenTarget::Tab => "A tab in this window",
            OpenTarget::Window => "A window of its own",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            OpenTarget::Tab => "Open the model in a tab of the window it was opened from.",
            OpenTarget::Window => "Open the model in a window of its own, so two models can be worked on side by side.",
        }
    }
}
