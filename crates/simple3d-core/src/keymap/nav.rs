//! The navigation presets a user can pick between.

use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavMap {
    pub orbit: Drag,
    pub pan: Drag,
    /// Some users, and some other programs, scroll the other way.
    pub invert_zoom: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    /// Simple 3D's own defaults: two buttons and a wheel, so a trackpad or a
    /// two-button mouse is enough.
    Default,
    /// The common mesh-editor convention: middle-drag orbits, G/R/S switch the
    /// manipulator mode.
    MeshEditor,
    /// The common CAD convention: middle-drag orbits, Ctrl+middle pans, and the
    /// wheel zooms the other way.
    Cad,
}

impl Preset {
    pub const ALL: [Preset; 3] = [Preset::Default, Preset::MeshEditor, Preset::Cad];

    pub fn label(self) -> &'static str {
        match self {
            Preset::Default => "Simple 3D default",
            Preset::MeshEditor => "Mesh editor",
            Preset::Cad => "CAD",
        }
    }
}
