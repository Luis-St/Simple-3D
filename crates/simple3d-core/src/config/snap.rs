//! What a drag snaps to, and the frame its handles stand in.

use serde::{Deserialize, Serialize};

/// When a drag snaps to the geometry of other bodies -- their vertices, edge
/// midpoints and face centres -- rather than only to the grid step (issue 68).
///
/// The three answers are the three a modelling tool always ends up wanting:
/// someone placing parts against each other wants it always on, someone laying
/// out a field of shapes wants it never on, and most of the time the honest
/// answer is "when I ask", which is what holding a key gives.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapMode {
    /// Snap only while the snap key is held down.
    #[default]
    WhileHeld,
    /// Always snap to nearby geometry.
    Always,
    /// Never snap to geometry; the grid step is the only snap.
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

/// Which frame the manipulator handles work in (spec section 6.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HandleFrame {
    #[default]
    Object,
    World,
}

impl HandleFrame {
    pub fn label(self) -> &'static str {
        match self {
            HandleFrame::Object => "Object",
            HandleFrame::World => "World",
        }
    }

    pub fn toggled(self) -> HandleFrame {
        match self {
            HandleFrame::Object => HandleFrame::World,
            HandleFrame::World => HandleFrame::Object,
        }
    }
}
