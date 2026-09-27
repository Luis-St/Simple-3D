//! Camera maths for the viewport: the eye, projecting world points, and screen rays for picking
//! and handle drags (spec section 6.1).

mod camera;
mod preset;
mod project;
pub use preset::{shortest_turn, CameraMove, ViewPreset};
// Only the tests need the transition's length.
#[cfg(test)]
pub(crate) use preset::TRANSITION;
mod cube;
pub use cube::{cube_project, cube_zone_angles, cube_zone_at, cube_zone_label, zone_order, CUBE_FACES};
mod frame;
pub use frame::frame_bounds;
#[cfg(test)]
mod tests;

use simple3d_core::scene::Camera;
use simple3d_geom::Vec3;

/// A camera bound to a screen rectangle, rebuilt every frame. The projection is computed once when
/// the view is made, since deriving it per point was most of a large frame's preparation; hence the
/// camera is read-only, and a moved camera gets a new view.
#[derive(Clone, Copy, Debug)]
pub struct View {
    camera: Camera,
    pub centre: egui::Pos2,
    pub size: egui::Vec2,
    /// Screen right and up in world space, the view direction, the eye, and pixels per millimetre.
    projection: Projection,
}

/// The projection settled by the camera and panel height (see [`View`]).
#[derive(Clone, Copy, Debug)]
struct Projection {
    right: Vec3,
    up: Vec3,
    forward: Vec3,
    eye: Vec3,
    pixels_per_mm: f64,
}
