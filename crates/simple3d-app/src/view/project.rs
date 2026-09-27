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

    /// View-space point to screen pixels, with the view-space depth (positive in front of the eye).
    pub fn view_to_screen(&self, v: Vec3) -> (egui::Pos2, f64) {
        let s = self.pixels_per_mm();
        let (x, y) = (v.x * s, v.y * s);
        (egui::pos2(self.centre.x + x as f32, self.centre.y - y as f32), v.z)
    }

    /// Where a world point lands; always `Some` under orthographic projection.
    pub fn project(&self, world: Vec3) -> Option<(egui::Pos2, f64)> {
        Some(self.view_to_screen(self.to_view(world)))
    }

    /// The ray through a screen position: `(origin, direction)`, direction normalised and constant.
    pub fn ray(&self, screen: egui::Pos2) -> (Vec3, Vec3) {
        let (right, up) = self.basis();
        let dx = (screen.x - self.centre.x) as f64;
        let dy = (self.centre.y - screen.y) as f64;
        let s = self.pixels_per_mm();
        (self.eye() + right * (dx / s) + up * (dy / s), self.forward())
    }

    /// World units per screen pixel, for constant-size handles and pixel-to-millimetre drags.
    pub fn mm_per_pixel_at(&self, _world: Vec3) -> f64 {
        1.0 / self.pixels_per_mm().max(1e-9)
    }

    /// Where a screen ray meets a plane through `origin` with normal `normal`; `None` if parallel. The
    /// hit counts anywhere along the ray, even behind the camera plane, so a drag never loses its plane.
    pub fn ray_plane(&self, screen: egui::Pos2, origin: Vec3, normal: Vec3) -> Option<Vec3> {
        let (ro, rd) = self.ray(screen);
        let denom = rd.dot(normal);
        if denom.abs() < 1e-9 {
            return None;
        }
        Some(ro + rd * ((origin - ro).dot(normal) / denom))
    }

    /// The same hit, but only in front of the camera: pixels above the horizon meet the ground behind
    /// the viewer, and are sky.
    pub fn ray_plane_ahead(&self, screen: egui::Pos2, origin: Vec3, normal: Vec3) -> Option<Vec3> {
        let (ro, rd) = self.ray(screen);
        let denom = rd.dot(normal);
        if denom.abs() < 1e-9 {
            return None;
        }
        let t = (origin - ro).dot(normal) / denom;
        (t >= 0.0).then(|| ro + rd * t)
    }

    /// The closest point on the line through `origin` along `axis` to a screen ray, as a distance
    /// along the axis: what an axis-arrow drag solves.
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
