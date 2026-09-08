//! The settings a scene carries with it, so a reopened project looks the
//! way it was left.

use super::*;
use crate::unit::Unit;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SceneSettings {
    pub unit: Unit,
    pub default_segments: u32,
    #[serde(default)]
    pub notes: String,
    pub grid_spacing: f64,
    pub grid_visible: bool,
    /// How far one step of a move or resize goes: the increment a drag snaps to
    /// and one press of a nudge key covers. Its own setting rather than the grid
    /// spacing, which is about what the ground looks like -- 1 mm is the step
    /// most people want and a 1 mm grid is unreadable.
    #[serde(default = "default_snap_step")]
    pub snap_step: f64,
    /// The three origin axes, each on its own. An axis running through the model
    /// is a distraction when it is not the one being worked to.
    #[serde(default = "all_axes")]
    pub axes_visible: [bool; 3],
    #[serde(default)]
    pub axis_style: AxisStyle,
    /// Draw, on the surface of a solid, the line where a principal plane cuts
    /// through it. Where the ground plane crosses a shape is a real dimension
    /// -- how much of it is below the build plate -- and it is invisible until
    /// something marks it.
    #[serde(default = "default_true")]
    pub plane_marks: bool,
    /// What the viewport does while a tool draws a preview over it (issue 82).
    /// Absent from the file while it is the default, so a project written by
    /// this version still diffs cleanly against one written before in-place
    /// previews existed.
    #[serde(default, skip_serializing_if = "is_no_change")]
    pub preview_viewport: PreviewViewport,
    /// The plane the model is cut with on screen (issue 71). Off, and absent
    /// from the file, until it is asked for.
    #[serde(default, skip_serializing_if = "is_off")]
    pub section: SectionView,
}

impl Default for SceneSettings {
    fn default() -> Self {
        SceneSettings {
            unit: Unit::Millimetre,
            // 32 segments keeps a 3mm pin smooth and a 2m cylinder acceptable
            // without the user touching the setting (spec section 5.1).
            default_segments: 32,
            notes: String::new(),
            grid_spacing: 10.0,
            grid_visible: true,
            snap_step: default_snap_step(),
            axes_visible: all_axes(),
            axis_style: AxisStyle::default(),
            plane_marks: true,
            preview_viewport: PreviewViewport::NoChange,
            section: SectionView::default(),
        }
    }
}
