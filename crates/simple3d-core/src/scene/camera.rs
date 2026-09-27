//! Where the scene is viewed from.

use serde::{Deserialize, Serialize};
use simple3d_geom::Vec3;

/// Saved with the project (spec section 6.1). Always orthographic: perspective fans out grid lines,
/// axes and box edges, and measurements here are typed, not judged by eye. An old `orthographic`
/// flag is ignored on load.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    pub target: Vec3,
    pub distance: f64,
    /// Degrees around Z.
    pub yaw: f64,
    /// Degrees above the XY plane.
    pub pitch: f64,
    /// The zoom, as the field of view a perspective camera at `distance` would need: `distance` times
    /// the tangent of half this is the frame's half-height in millimetres.
    pub fov_deg: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Camera { target: Vec3::ZERO, distance: 160.0, yaw: -55.0, pitch: 28.0, fov_deg: 45.0 }
    }
}
