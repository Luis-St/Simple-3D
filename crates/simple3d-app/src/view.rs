//! Camera maths for the viewport: where the eye is, how a world point lands on
//! screen, and how a screen point turns back into a ray for picking and for
//! dragging a handle (spec section 6.1).

mod camera;
mod preset;
mod project;
pub use preset::{shortest_turn, CameraMove, ViewPreset};
// Only the tests reach for the transition's length.
#[cfg(test)]
pub(crate) use preset::TRANSITION;
mod cube;
pub use cube::{cube_project, cube_zone_angles, cube_zone_at, cube_zone_label, zone_order, CUBE_FACES};
mod frame;
pub use frame::frame_bounds;
#[cfg(test)]
mod tests;

use simple3d_core::scene::Camera;

/// A camera bound to a particular on-screen rectangle. Cheap to build, so it is
/// rebuilt every frame from the scene's camera and the panel's current size.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub camera: Camera,
    pub centre: egui::Pos2,
    pub size: egui::Vec2,
}
