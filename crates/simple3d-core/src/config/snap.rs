//! What a drag snaps to.

use serde::{Deserialize, Serialize};

/// When a drag snaps to other bodies' vertices, edge midpoints and face centres, not only the grid
/// step (issue 68).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapMode {
    /// Snap only while the snap key is held.
    #[default]
    WhileHeld,
    Always,
    /// Never snap to geometry; only the grid step.
    Never,
}

impl SnapMode {
    pub const ALL: [SnapMode; 3] = [SnapMode::WhileHeld, SnapMode::Always, SnapMode::Never];

    pub fn label(self) -> &'static str {
        match self {
            SnapMode::WhileHeld => "While a key is held",
            SnapMode::Always => "Always",
            SnapMode::Never => "Never",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            SnapMode::WhileHeld => {
                "Snap a drag onto another body's vertices, edge midpoints and face centres \
                 while the snap key is held down."
            }
            SnapMode::Always => "Snap every drag onto whatever body feature the pointer is over.",
            SnapMode::Never => "Snap drags to the document's step and nothing else.",
        }
    }
}
