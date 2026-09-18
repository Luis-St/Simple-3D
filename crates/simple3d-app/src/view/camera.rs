//! The camera's frame: where it stands and which way it faces.

use super::*;
use simple3d_core::scene::Camera;
use simple3d_geom::Vec3;

impl View {
    pub fn new(camera: Camera, rect: egui::Rect) -> View {
        let (centre, size) = (rect.center(), rect.size());
        View { camera, centre, size, projection: Projection::of(camera, size) }
    }

    /// The same rectangle seen from another camera. How a camera is changed,
    /// since assigning to one would leave the projection behind -- see [`View`].
    /// Only the tests move a camera without the application building the view
    /// afresh from the scene's own, which is what every frame does.
    #[cfg(test)]
    pub fn with_camera(&self, camera: Camera) -> View {
        View { camera, centre: self.centre, size: self.size, projection: Projection::of(camera, self.size) }
    }

    pub fn camera(&self) -> Camera {
        self.camera
    }

    /// Direction from the target towards the eye. The tests' way of putting a
    /// shape between the camera and the origin; the renderer asks for
    /// [`View::forward`] instead.
    #[cfg(test)]
    pub fn offset_dir(&self) -> Vec3 {
        -self.projection.forward
    }

    pub fn eye(&self) -> Vec3 {
        self.projection.eye
    }

    pub fn forward(&self) -> Vec3 {
        self.projection.forward
    }

    /// Screen right and up, in world space.
    pub fn basis(&self) -> (Vec3, Vec3) {
        (self.projection.right, self.projection.up)
    }

    /// Pixels per world unit: the projection's whole scale, since it is
    /// orthographic and one millimetre is the same number of pixels wherever it
    /// sits in the frame.
    pub fn pixels_per_mm(&self) -> f64 {
        self.projection.pixels_per_mm
    }
}

impl Projection {
    fn of(camera: Camera, size: egui::Vec2) -> Projection {
        let yaw = camera.yaw.to_radians();
        let pitch = camera.pitch.to_radians().clamp(-1.5533, 1.5533);
        let offset_dir = Vec3::new(pitch.cos() * yaw.cos(), pitch.cos() * yaw.sin(), pitch.sin());
        let forward = -offset_dir;
        let mut right = forward.cross(Vec3::new(0.0, 0.0, 1.0));
        if right.length() < 1e-9 {
            // Looking straight down or up: any horizontal axis will do, and
            // picking one keeps the view usable instead of collapsing.
            right = Vec3::new(1.0, 0.0, 0.0);
        }
        let right = right.normalized();
        let half_height = camera.distance * (camera.fov_deg.to_radians() / 2.0).tan();
        Projection {
            right,
            up: right.cross(forward).normalized(),
            forward,
            eye: camera.target + offset_dir * camera.distance,
            pixels_per_mm: (size.y as f64 / 2.0) / half_height.max(1e-9),
        }
    }
}
