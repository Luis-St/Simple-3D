//! The navigation presets.

use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavMap {
    pub orbit: Drag,
    pub pan: Drag,
    /// Some users and programs scroll the other way.
    pub invert_zoom: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    /// Simple 3D's defaults: two buttons and a wheel, enough for a trackpad.
    Default,
    /// Mesh-editor convention: middle-drag orbits, G/R/S switch manipulator mode.
    MeshEditor,
    /// CAD convention: middle-drag orbits, Ctrl+middle pans, wheel zooms reversed.
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
