//! Between world space and the screen, and back along a ray.

use super::*;
use simple3d_geom::Vec3;

impl View {
    /// World point to view space: X right, Y up, Z forward (depth).
    pub fn to_view(&self, world: Vec3) -> Vec3 {
        let (right, up) = self.basis();
        let d = world - self.eye();
        Vec3::new(d.dot(right), d.dot(up), d.dot(self.forward()))
    }

    /// View-space point to screen pixels. Returns the depth alongside, which is
    /// view-space Z -- positive for anything in front of the eye.
    pub fn view_to_screen(&self, v: Vec3) -> (egui::Pos2, f64) {
        let s = self.pixels_per_mm();
        let (x, y) = (v.x * s, v.y * s);
        (egui::pos2(self.centre.x + x as f32, self.centre.y - y as f32), v.z)
    }

    /// Where a world point lands. `Some` for every point, orthographic
    /// projection having no near plane to fall behind; the option is kept
    /// because callers read better for asking.
    pub fn project(&self, world: Vec3) -> Option<(egui::Pos2, f64)> {
        Some(self.view_to_screen(self.to_view(world)))
    }

    /// Ray through a screen position: `(origin, direction)`, direction normalised.
    /// Every ray runs along the view direction; only where it starts changes.
    pub fn ray(&self, screen: egui::Pos2) -> (Vec3, Vec3) {
        let (right, up) = self.basis();
        let dx = (screen.x - self.centre.x) as f64;
        let dy = (self.centre.y - screen.y) as f64;
        let s = self.pixels_per_mm();
        (self.eye() + right * (dx / s) + up * (dy / s), self.forward())
    }

    /// How many world units one screen pixel covers at `world`. Used to keep
    /// handles a constant on-screen size regardless of zoom, and to convert a
    /// drag in pixels into a drag in millimetres.
    pub fn mm_per_pixel_at(&self, _world: Vec3) -> f64 {
        1.0 / self.pixels_per_mm().max(1e-9)
    }

    /// Where a screen ray meets a plane through `origin` with normal `normal`.
    /// `None` when the ray runs parallel to it.
    ///
    /// A drag uses this and must never fail on the plane it grabbed, so the hit
    /// counts wherever it is along the ray -- including behind the camera plane,
    /// which is where a handle ends up when the view is zoomed right into it.
    pub fn ray_plane(&self, screen: egui::Pos2, origin: Vec3, normal: Vec3) -> Option<Vec3> {
        let (ro, rd) = self.ray(screen);
        let denom = rd.dot(normal);
        if denom.abs() < 1e-9 {
            return None;
        }
        Some(ro + rd * ((origin - ro).dot(normal) / denom))
    }

    /// The same hit, but only when it lies in front of the camera.
    ///
    /// This is what "the pointer is on the ground" means, and it is not the same
    /// question: a parallel projection meets the ground plane for every pixel of
    /// the frame, including the ones above the horizon, where the meeting point
    /// is behind the viewer. Those pixels are sky, and clicking one has to mean
    /// what it looks like it means.
    pub fn ray_plane_ahead(&self, screen: egui::Pos2, origin: Vec3, normal: Vec3) -> Option<Vec3> {
        let (ro, rd) = self.ray(screen);
        let denom = rd.dot(normal);
        if denom.abs() < 1e-9 {
            return None;
        }
        let t = (origin - ro).dot(normal) / denom;
        (t >= 0.0).then(|| ro + rd * t)
    }

    /// The closest point to a screen ray on the line through `origin` along
    /// `axis`, as a distance along that axis. This is what an axis-arrow drag
    /// solves: the handle follows the cursor while staying on its axis.
    pub fn ray_axis(&self, screen: egui::Pos2, origin: Vec3, axis: Vec3) -> Option<f64> {
        let (ro, rd) = self.ray(screen);
        let axis = axis.normalized();
        let w = origin - ro;
        let a = axis.dot(axis);
        let b = axis.dot(rd);
        let c = rd.dot(rd);
        let d = axis.dot(w);
        let e = rd.dot(w);
        let denom = a * c - b * b;
        if denom.abs() < 1e-12 {
            return None;
        }
        Some((b * e - c * d) / denom)
    }
}
