//! Where the scene is viewed from.

use serde::{Deserialize, Serialize};
use simple3d_geom::Vec3;

/// Saved with the project (spec section 6.1: "The camera position is part of
/// the saved project").
///
/// The projection is always orthographic. A perspective view converges every
/// parallel line, which in a modelling tool means the grid lines, the origin
/// axes and the edges of a box all fan out from one another instead of running
/// together -- a box on the origin was drawn with the axes crossing its top
/// face at a visibly different angle from the grid they lie on. Nothing here is
/// judged by eye; every measurement is typed and read back, so the projection
/// that keeps parallels parallel is the only one worth having. Older files
/// carrying an `orthographic` flag still load: the field is simply ignored.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    pub target: Vec3,
    pub distance: f64,
    /// Degrees around Z.
    pub yaw: f64,
    /// Degrees above the XY plane.
    pub pitch: f64,
    /// How much of the scene the frame covers, expressed as the field of view a
    /// perspective camera at `distance` would need to cover the same height.
    /// Under orthographic projection it is one half of the zoom: `distance`
    /// times the tangent of half of this is the half-height of the frame in
    /// millimetres.
    pub fov_deg: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Camera { target: Vec3::ZERO, distance: 160.0, yaw: -55.0, pitch: 28.0, fov_deg: 45.0 }
    }
}
