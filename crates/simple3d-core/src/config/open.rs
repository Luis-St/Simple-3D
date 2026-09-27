//! Where a model opened in a running application goes.

use serde::{Deserialize, Serialize};

/// What opening a model does with a document already open (issue 107): a tab by default, or a window
/// of its own for side-by-side or per-display work.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenTarget {
    /// A tab in the window the file was opened from.
    #[default]
    Tab,
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
