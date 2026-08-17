//! Camera maths for the viewport: where the eye is, how a world point lands on
//! screen, and how a screen point turns back into a ray for picking and for
//! dragging a handle (spec section 6.1).

use scadstudio_core::scene::Camera;
use scadstudio_geom::Vec3;

/// A camera bound to a particular on-screen rectangle. Cheap to build, so it is
/// rebuilt every frame from the scene's camera and the panel's current size.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub camera: Camera,
    pub centre: egui::Pos2,
    pub size: egui::Vec2,
}

/// Everything nearer than this is behind the eye as far as projection is
/// concerned; triangles are clipped against it so the preview does not explode
/// when the camera is inside the model.
pub const NEAR: f64 = 0.05;

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

    /// Pixels per world unit at the target plane. Also the orthographic scale.
    pub fn pixels_per_mm(&self) -> f64 {
        let half_height = self.camera.distance * (self.camera.fov_deg.to_radians() / 2.0).tan();
        (self.size.y as f64 / 2.0) / half_height.max(1e-9)
    }

    /// Focal length in pixels for the perspective projection.
    fn focal(&self) -> f64 {
        (self.size.y as f64 / 2.0) / (self.camera.fov_deg.to_radians() / 2.0).tan()
    }

    /// World point to view space: X right, Y up, Z forward (depth).
    pub fn to_view(&self, world: Vec3) -> Vec3 {
        let (right, up) = self.basis();
        let d = world - self.eye();
        Vec3::new(d.dot(right), d.dot(up), d.dot(self.forward()))
    }

    /// View-space point to screen pixels. Returns the depth alongside, which is
    /// view-space Z -- always positive for anything in front of the eye.
    pub fn view_to_screen(&self, v: Vec3) -> (egui::Pos2, f64) {
        let (x, y) = if self.camera.orthographic {
            let s = self.pixels_per_mm();
            (v.x * s, v.y * s)
        } else {
            let f = self.focal();
            let z = v.z.max(NEAR);
            (v.x * f / z, v.y * f / z)
        };
        (egui::pos2(self.centre.x + x as f32, self.centre.y - y as f32), v.z)
    }

    /// `None` when the point is behind the near plane, so callers do not draw a
    /// mirrored ghost of geometry behind the eye.
    pub fn project(&self, world: Vec3) -> Option<(egui::Pos2, f64)> {
        let v = self.to_view(world);
        if !self.camera.orthographic && v.z <= NEAR {
            return None;
        }
        Some(self.view_to_screen(v))
    }

    /// Ray through a screen position: `(origin, direction)`, direction normalised.
    pub fn ray(&self, screen: egui::Pos2) -> (Vec3, Vec3) {
        let (right, up) = self.basis();
        let dx = (screen.x - self.centre.x) as f64;
        let dy = (self.centre.y - screen.y) as f64;
        if self.camera.orthographic {
            let s = self.pixels_per_mm();
            let origin = self.eye() + right * (dx / s) + up * (dy / s);
            (origin, self.forward())
        } else {
            let f = self.focal();
            let dir = (right * (dx / f) + up * (dy / f) + self.forward()).normalized();
            (self.eye(), dir)
        }
    }

    /// How many world units one screen pixel covers at `world`. Used to keep
    /// handles a constant on-screen size regardless of zoom, and to convert a
    /// drag in pixels into a drag in millimetres.
    pub fn mm_per_pixel_at(&self, world: Vec3) -> f64 {
        if self.camera.orthographic {
            1.0 / self.pixels_per_mm().max(1e-9)
        } else {
            let z = self.to_view(world).z.max(NEAR);
            z / self.focal().max(1e-9)
        }
    }

    /// Where a screen ray meets a plane through `origin` with normal `normal`.
    /// `None` when the ray runs parallel to it.
    pub fn ray_plane(&self, screen: egui::Pos2, origin: Vec3, normal: Vec3) -> Option<Vec3> {
        let (ro, rd) = self.ray(screen);
        let denom = rd.dot(normal);
        if denom.abs() < 1e-9 {
            return None;
        }
        let t = (origin - ro).dot(normal) / denom;
        if t < 0.0 && !self.camera.orthographic {
            return None;
        }
        Some(ro + rd * t)
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

/// The standard view presets (spec section 6.1). Yaw and pitch in degrees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewPreset {
    Top,
    Bottom,
    Front,
    Back,
    Left,
    Right,
    Isometric,
}

impl ViewPreset {
    /// Yaw and pitch that put the camera on the named side. The camera looks
    /// along `-offset_dir`, so "front" (looking at the XZ plane from -Y) means
    /// the eye sits at -Y, i.e. yaw = -90 degrees.
    pub fn angles(self) -> (f64, f64) {
        match self {
            ViewPreset::Top => (-90.0, 89.9),
            ViewPreset::Bottom => (-90.0, -89.9),
            ViewPreset::Front => (-90.0, 0.0),
            ViewPreset::Back => (90.0, 0.0),
            ViewPreset::Right => (0.0, 0.0),
            ViewPreset::Left => (180.0, 0.0),
            ViewPreset::Isometric => (-55.0, 28.0),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ViewPreset::Top => "Top",
            ViewPreset::Bottom => "Bottom",
            ViewPreset::Front => "Front",
            ViewPreset::Back => "Back",
            ViewPreset::Left => "Left",
            ViewPreset::Right => "Right",
            ViewPreset::Isometric => "Isometric",
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> View {
        let camera = Camera { yaw: -90.0, pitch: 0.0, distance: 100.0, ..Camera::default() };
        View::new(camera, egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0)))
    }

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-6
    }

    #[test]
    fn the_front_view_looks_along_positive_y() {
        let v = view();
        assert!(close(v.eye(), Vec3::new(0.0, -100.0, 0.0)), "{:?}", v.eye());
        assert!(close(v.forward(), Vec3::new(0.0, 1.0, 0.0)), "{:?}", v.forward());
        let (right, up) = v.basis();
        assert!(close(right, Vec3::new(1.0, 0.0, 0.0)), "{right:?}");
        assert!(close(up, Vec3::new(0.0, 0.0, 1.0)), "{up:?}");
    }

    #[test]
    fn the_target_projects_to_the_centre_of_the_viewport() {
        let v = view();
        let (screen, depth) = v.project(v.camera.target).unwrap();
        assert!((screen - v.centre).length() < 1e-3, "{screen:?} vs {:?}", v.centre);
        assert!((depth - 100.0).abs() < 1e-6);
    }

    #[test]
    fn screen_right_is_world_right_and_screen_up_is_world_up() {
        let v = view();
        let right = v.project(Vec3::new(10.0, 0.0, 0.0)).unwrap().0;
        assert!(right.x > v.centre.x, "+X should be to the right");
        let up = v.project(Vec3::new(0.0, 0.0, 10.0)).unwrap().0;
        assert!(up.y < v.centre.y, "+Z should be up (screen Y grows downward)");
    }

    #[test]
    fn projecting_and_unprojecting_agree() {
        for ortho in [false, true] {
            let mut v = view();
            v.camera.orthographic = ortho;
            v.camera.yaw = -55.0;
            v.camera.pitch = 28.0;
            for world in [
                Vec3::new(10.0, 5.0, 3.0),
                Vec3::new(-30.0, 12.0, -8.0),
                Vec3::new(0.0, 0.0, 0.0),
            ] {
                let (screen, _) = v.project(world).unwrap();
                let (origin, dir) = v.ray(screen);
                // The world point must lie on the ray through its own projection.
                let along = (world - origin).dot(dir);
                let closest = origin + dir * along;
                // Screen coordinates are f32, so a round trip through them is
                // good to a fraction of a micrometre, not to the last bit.
                assert!(
                    (closest - world).length() < 1e-3,
                    "ortho={ortho} {world:?} -> {screen:?} -> off by {}",
                    (closest - world).length()
                );
            }
        }
    }

    #[test]
    fn points_behind_the_eye_do_not_project_in_perspective() {
        let v = view();
        assert!(v.project(Vec3::new(0.0, -200.0, 0.0)).is_none());
        // In orthographic there is no near-plane singularity, so it still maps.
        let mut ortho = v;
        ortho.camera.orthographic = true;
        assert!(ortho.project(Vec3::new(0.0, -200.0, 0.0)).is_some());
    }

    #[test]
    fn an_axis_drag_solves_for_distance_along_the_axis() {
        let v = view();
        let origin = Vec3::ZERO;
        let axis = Vec3::new(1.0, 0.0, 0.0);
        let target = Vec3::new(25.0, 0.0, 0.0);
        let (screen, _) = v.project(target).unwrap();
        let along = v.ray_axis(screen, origin, axis).unwrap();
        assert!((along - 25.0).abs() < 1e-3, "got {along}");
    }

    #[test]
    fn a_plane_drag_lands_on_the_plane() {
        let v = view();
        let hit = v.ray_plane(v.centre + egui::vec2(60.0, -30.0), Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0)).unwrap();
        assert!(hit.y.abs() < 1e-6, "not on the plane: {hit:?}");
        assert!(hit.x > 0.0 && hit.z > 0.0, "{hit:?}");
    }

    #[test]
    fn a_pixel_covers_more_millimetres_as_the_camera_pulls_back() {
        let mut v = view();
        let near = v.mm_per_pixel_at(Vec3::ZERO);
        v.camera.distance = 400.0;
        let far = v.mm_per_pixel_at(Vec3::ZERO);
        assert!(far > near * 3.5, "{near} -> {far}");
    }

    #[test]
    fn framing_bounds_centres_and_fits_them() {
        let mut camera = Camera::default();
        let (lo, hi) = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(40.0, 20.0, 4.0));
        frame_bounds(&mut camera, lo, hi, 800.0 / 600.0);
        assert!(close(camera.target, Vec3::new(20.0, 10.0, 2.0)), "{:?}", camera.target);

        let v = View::new(camera, egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)));
        // Every corner is on screen, and the object is not lost in the middle.
        for corner in [lo, hi, Vec3::new(lo.x, hi.y, lo.z), Vec3::new(hi.x, lo.y, hi.z)] {
            let (screen, _) = v.project(corner).expect("corner behind the camera");
            assert!(screen.x > 0.0 && screen.x < 800.0 && screen.y > 0.0 && screen.y < 600.0, "{screen:?}");
        }
    }

    #[test]
    fn looking_straight_down_still_yields_a_usable_basis() {
        let mut v = view();
        v.camera.pitch = 90.0;
        let (right, up) = v.basis();
        assert!(right.length() > 0.9 && up.length() > 0.9);
        assert!(right.dot(up).abs() < 1e-6, "basis is not orthogonal");
    }

    #[test]
    fn every_view_preset_has_distinct_angles() {
        let mut seen: Vec<(i64, i64)> = Vec::new();
        for preset in [
            ViewPreset::Top,
            ViewPreset::Bottom,
            ViewPreset::Front,
            ViewPreset::Back,
            ViewPreset::Left,
            ViewPreset::Right,
            ViewPreset::Isometric,
        ] {
            let (yaw, pitch) = preset.angles();
            let key = ((yaw * 10.0) as i64, (pitch * 10.0) as i64);
            assert!(!seen.contains(&key), "{:?} duplicates another preset", preset.label());
            seen.push(key);
        }
    }

    #[test]
    fn the_top_preset_looks_down() {
        let mut v = view();
        let (yaw, pitch) = ViewPreset::Top.angles();
        v.camera.yaw = yaw;
        v.camera.pitch = pitch;
        assert!(v.forward().z < -0.99, "{:?}", v.forward());
        assert!(v.eye().z > 90.0);
    }
}
