//! Framing the camera on a bounding box.

use simple3d_core::scene::Camera;
use simple3d_geom::Vec3;

/// Move and zoom the camera so `bounds` fills the viewport with a little margin
/// (frame-selection and frame-all).
pub fn frame_bounds(camera: &mut Camera, lo: Vec3, hi: Vec3, aspect: f64) {
    let centre = (lo + hi) * 0.5;
    let radius = ((hi - lo).length() / 2.0).max(1.0);
    camera.target = centre;
    let half_fov = (camera.fov_deg.to_radians() / 2.0).tan().max(1e-6);
    // Fit the bounding sphere in the narrower of the two screen axes.
    let shrink = if aspect < 1.0 { aspect } else { 1.0 };
    camera.distance = (radius / (half_fov * shrink)) * 1.25;
}
