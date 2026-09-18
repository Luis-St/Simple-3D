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
use simple3d_geom::Vec3;

/// A camera bound to a particular on-screen rectangle. Cheap to build, so it is
/// rebuilt every frame from the scene's camera and the panel's current size.
///
/// The projection itself -- where the eye stands, which way is screen right, up
/// and forward, and how many pixels a millimetre is -- is worked out when the
/// view is made and kept. It used to be derived on demand, and `to_view` asks
/// for three of those, so projecting a single point cost two rounds of
/// trigonometry and three square roots; a frame projects hundreds of thousands
/// of points, and on an imported assembly that arithmetic was most of what the
/// viewport spent preparing one.
///
/// That is why the camera is read-only. A view whose camera had been assigned
/// to would still be carrying the projection the old one gave it, and every
/// point would land where the camera used to be. A camera that has moved gets a
/// view of its own instead, which is what the viewport builds every frame.
#[derive(Clone, Copy, Debug)]
pub struct View {
    camera: Camera,
    pub centre: egui::Pos2,
    pub size: egui::Vec2,
    /// Screen right and up in world space, the direction the camera looks, the
    /// eye's position, and the projection's scale in pixels per millimetre.
    projection: Projection,
}

/// Everything about the projection that the camera and the panel's height
/// settle between them -- see [`View`] for why it is settled once.
#[derive(Clone, Copy, Debug)]
struct Projection {
    right: Vec3,
    up: Vec3,
    forward: Vec3,
    eye: Vec3,
    pixels_per_mm: f64,
}
