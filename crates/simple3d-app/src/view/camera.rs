//! The camera's frame: where it stands and which way it faces.

use super::*;
use simple3d_core::scene::Camera;
use simple3d_geom::Vec3;

impl View {
    pub fn new(camera: Camera, rect: egui::Rect) -> View {
        View { camera, centre: rect.center(), size: rect.size() }
    }

    /// Direction from the target towards the eye.
    pub fn offset_dir(&self) -> Vec3 {
        let yaw = self.camera.yaw.to_radians();
        let pitch = self.camera.pitch.to_radians().clamp(-1.5533, 1.5533);
        Vec3::new(pitch.cos() * yaw.cos(), pitch.cos() * yaw.sin(), pitch.sin())
    }

    pub fn eye(&self) -> Vec3 {
        self.camera.target + self.offset_dir() * self.camera.distance
    }

    pub fn forward(&self) -> Vec3 {
        -self.offset_dir()
    }

    /// Screen right and up, in world space.
    pub fn basis(&self) -> (Vec3, Vec3) {
        let forward = self.forward();
        let mut right = forward.cross(Vec3::new(0.0, 0.0, 1.0));
        if right.length() < 1e-9 {
            // Looking straight down or up: any horizontal axis will do, and
            // picking one keeps the view usable instead of collapsing.
            right = Vec3::new(1.0, 0.0, 0.0);
        }
        let right = right.normalized();
        let up = right.cross(forward).normalized();
        (right, up)
    }

    /// Pixels per world unit: the projection's whole scale, since it is
    /// orthographic and one millimetre is the same number of pixels wherever it
    /// sits in the frame.
    pub fn pixels_per_mm(&self) -> f64 {
        let half_height = self.camera.distance * (self.camera.fov_deg.to_radians() / 2.0).tan();
        (self.size.y as f64 / 2.0) / half_height.max(1e-9)
    }
}
