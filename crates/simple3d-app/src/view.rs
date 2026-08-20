//! Camera maths for the viewport: where the eye is, how a world point lands on
//! screen, and how a screen point turns back into a ray for picking and for
//! dragging a handle (spec section 6.1).

use simple3d_core::scene::Camera;
use simple3d_geom::Vec3;

/// A camera bound to a particular on-screen rectangle. Cheap to build, so it is
/// rebuilt every frame from the scene's camera and the panel's current size.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub camera: Camera,
    pub centre: egui::Pos2,
    pub size: egui::Vec2,
}

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

/// The shortest way round from one yaw to another, in degrees. Turning from
/// 170 to -170 is twenty degrees, not three hundred and forty: a view cube that
/// spins the long way round to an adjacent face reads as a glitch.
pub fn shortest_turn(from: f64, to: f64) -> f64 {
    let mut delta = (to - from) % 360.0;
    if delta > 180.0 {
        delta -= 360.0;
    }
    if delta < -180.0 {
        delta += 360.0;
    }
    delta
}

/// The transition curve: ease in and out, so the camera starts and stops rather
/// than jumping into motion at full speed.
pub fn ease(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How long a view change takes. The design's number, and short enough that it
/// reads as the same camera moving rather than as a wait.
pub const TRANSITION: std::time::Duration = std::time::Duration::from_millis(200);

/// A camera turn in flight.
#[derive(Clone, Copy, Debug)]
pub struct CameraMove {
    pub from: (f64, f64),
    pub to: (f64, f64),
    pub started: std::time::Instant,
}

impl CameraMove {
    /// Yaw and pitch at this moment, and whether the move is over.
    pub fn at(&self, now: std::time::Instant) -> ((f64, f64), bool) {
        let t = now.duration_since(self.started).as_secs_f64() / TRANSITION.as_secs_f64();
        let done = t >= 1.0;
        let e = ease(t);
        ((self.from.0 + (self.to.0 - self.from.0) * e, self.from.1 + (self.to.1 - self.from.1) * e), done)
    }
}

/// The six faces of the orientation cube, as an outward normal and the view
/// each one gives. Driven from here so the cube and the View menu cannot come
/// to disagree about which way "front" is.
pub const CUBE_FACES: [([i32; 3], ViewPreset, &str); 6] = [
    ([1, 0, 0], ViewPreset::Right, "RGT"),
    ([-1, 0, 0], ViewPreset::Left, "LFT"),
    ([0, 1, 0], ViewPreset::Back, "BCK"),
    ([0, -1, 0], ViewPreset::Front, "FRT"),
    ([0, 0, 1], ViewPreset::Top, "TOP"),
    ([0, 0, -1], ViewPreset::Bottom, "BTM"),
];

/// Project a unit-cube direction onto the orientation cube's face, given the
/// camera's yaw and pitch. Returns the offset from the cube's centre in points,
/// scaled by `reach`, and a depth that is negative towards the eye.
///
/// This is the cube's own little projection rather than the viewport's, because
/// the cube is always drawn the same size whatever the camera distance and
/// whatever the projection mode.
pub fn cube_project(yaw_deg: f64, pitch_deg: f64, v: Vec3, reach: f32) -> (egui::Vec2, f64) {
    // The viewport's own basis, written out: screen right, screen up and the
    // direction the camera looks. Deriving it from the same two angles is what
    // makes the cube unable to disagree with the view behind it -- and the
    // previous cube, which rotated its own way, disagreed with it by a quarter
    // turn.
    let (y, p) = (yaw_deg.to_radians(), pitch_deg.to_radians());
    let right = Vec3::new(-y.sin(), y.cos(), 0.0);
    let up = Vec3::new(-y.cos() * p.sin(), -y.sin() * p.sin(), p.cos());
    let forward = Vec3::new(-p.cos() * y.cos(), -p.cos() * y.sin(), -p.sin());
    // Screen y grows downward, so up is negated. Depth is negative towards the
    // eye, which is what makes a face visible.
    (egui::vec2(v.dot(right) as f32, -v.dot(up) as f32) * reach, v.dot(forward))
}

/// Which face of the orientation cube a point `offset` from its centre falls
/// on: the nearest face centre among the faces that are turned towards the eye.
/// `None` when the point is outside the cube altogether.
///
/// Faces pointing away are excluded rather than being the nearest match, so a
/// click never turns the camera to the side of the cube you cannot see.
pub fn cube_face_at(yaw_deg: f64, pitch_deg: f64, offset: egui::Vec2, reach: f32) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (index, (normal, _, _)) in CUBE_FACES.iter().enumerate() {
        let n = Vec3::new(normal[0] as f64, normal[1] as f64, normal[2] as f64);
        let (at, depth) = cube_project(yaw_deg, pitch_deg, n, reach);
        if depth >= 0.0 {
            continue;
        }
        let distance = (offset - at).length();
        if distance > reach * 0.9 {
            continue;
        }
        if best.is_none_or(|(_, d)| distance < d) {
            best = Some((index, distance));
        }
    }
    best.map(|(index, _)| index)
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
        let mut v = view();
        v.camera.yaw = -55.0;
        v.camera.pitch = 28.0;
        for world in [Vec3::new(10.0, 5.0, 3.0), Vec3::new(-30.0, 12.0, -8.0), Vec3::new(0.0, 0.0, 0.0)] {
            let (screen, _) = v.project(world).unwrap();
            let (origin, dir) = v.ray(screen);
            // The world point must lie on the ray through its own projection.
            let along = (world - origin).dot(dir);
            let closest = origin + dir * along;
            // Screen coordinates are f32, so a round trip through them is
            // good to a fraction of a micrometre, not to the last bit.
            assert!(
                (closest - world).length() < 1e-3,
                "{world:?} -> {screen:?} -> off by {}",
                (closest - world).length()
            );
        }
    }

    #[test]
    fn a_point_behind_the_eye_still_projects_where_it_belongs() {
        // There is no near plane to fall behind: the projection is parallel, so
        // a point the camera has passed lands at its true screen position and
        // is simply behind everything else. Nothing has to be clipped, which is
        // what stops geometry disappearing when the camera is inside the model.
        let v = view();
        let behind = Vec3::new(0.0, -200.0, 0.0);
        let (screen, depth) = v.project(behind).unwrap();
        assert!((screen - v.centre).length() < 1e-3, "{screen:?}");
        assert!(depth < 0.0, "a point behind the eye should have negative depth, got {depth}");
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
    fn a_turn_takes_the_short_way_round() {
        assert_eq!(shortest_turn(170.0, -170.0), 20.0);
        assert_eq!(shortest_turn(-170.0, 170.0), -20.0);
        assert_eq!(shortest_turn(0.0, 90.0), 90.0);
        assert_eq!(shortest_turn(0.0, 180.0), 180.0);
        assert_eq!(shortest_turn(10.0, 10.0), 0.0);
        // However many times the camera has been orbited round.
        assert_eq!(shortest_turn(720.0 + 10.0, 20.0), 10.0);
    }

    #[test]
    fn a_transition_starts_where_it_was_and_ends_where_it_was_asked_for() {
        let started = std::time::Instant::now();
        let m = CameraMove { from: (0.0, 0.0), to: (-90.0, 30.0), started };
        let ((yaw, pitch), done) = m.at(started);
        assert_eq!((yaw, pitch), (0.0, 0.0));
        assert!(!done);
        let ((yaw, pitch), done) = m.at(started + TRANSITION);
        assert!((yaw + 90.0).abs() < 1e-9 && (pitch - 30.0).abs() < 1e-9, "{yaw} {pitch}");
        assert!(done, "the move never finished");
        // And it eases: halfway through the time is halfway through the turn,
        // but a quarter of the way through is less than a quarter of the turn.
        let quarter = m.at(started + TRANSITION / 4).0 .0;
        assert!(quarter > -22.5, "the curve does not ease in: {quarter}");
    }

    #[test]
    fn the_cube_and_the_viewport_agree_about_which_way_is_which() {
        // The previous cube rotated its own way and disagreed with the view
        // behind it by a quarter turn, which makes an orientation cube worse
        // than none. Both are driven from yaw and pitch, so this can be checked
        // directly rather than looked at.
        for (yaw, pitch) in [(-55.0, 28.0), (-90.0, 0.0), (0.0, 0.0), (140.0, -35.0), (20.0, 60.0)] {
            let mut v = view();
            v.camera.yaw = yaw;
            v.camera.pitch = pitch;
            for axis in [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)] {
                let (screen, _) = v.project(v.camera.target + axis * 10.0).unwrap();
                let on_screen = screen - v.centre;
                let (on_cube, depth) = cube_project(yaw, pitch, axis, 20.0);
                assert_eq!(
                    on_screen.x.abs() < 1e-3,
                    on_cube.x.abs() < 1e-3,
                    "{axis:?} at yaw {yaw} pitch {pitch}: one says edge-on, the other does not"
                );
                if on_screen.x.abs() > 1e-3 {
                    assert_eq!(on_screen.x > 0.0, on_cube.x > 0.0, "{axis:?} points the other way across");
                }
                if on_screen.y.abs() > 1e-3 {
                    assert_eq!(on_screen.y > 0.0, on_cube.y > 0.0, "{axis:?} points the other way up");
                }
                // And an axis pointing towards the eye is the one drawn in
                // front: negative depth on the cube, nearer in the viewport. An
                // axis lying edge-on has no side to be on, so it is skipped.
                if depth.abs() > 1e-6 {
                    let nearer = v.to_view(v.camera.target + axis * 10.0).z < v.to_view(v.camera.target).z;
                    assert_eq!(
                        nearer,
                        depth < 0.0,
                        "{axis:?} at yaw {yaw} pitch {pitch} is drawn on the wrong side of the cube"
                    );
                }
            }
        }
    }

    #[test]
    fn clicking_a_face_of_the_cube_asks_for_the_view_that_face_shows() {
        let reach = 20.0_f32;
        for (yaw, pitch) in [ViewPreset::Isometric.angles(), (-30.0, 15.0), (120.0, -40.0)] {
            for (index, (normal, _, label)) in CUBE_FACES.iter().enumerate() {
                let n = Vec3::new(normal[0] as f64, normal[1] as f64, normal[2] as f64);
                let (at, depth) = cube_project(yaw, pitch, n, reach);
                if depth >= 0.0 {
                    // Turned away: it must not be clickable at all, or the cube
                    // would answer for a face nobody can see.
                    continue;
                }
                assert_eq!(cube_face_at(yaw, pitch, at, reach), Some(index), "{label} at yaw {yaw} pitch {pitch}");
            }
        }
    }

    #[test]
    fn a_face_turned_away_is_never_what_a_click_lands_on() {
        // Straight down the +Y axis: the back face is dead behind the front one,
        // and a click in the middle must be the front.
        let (yaw, pitch) = ViewPreset::Front.angles();
        let reach = 20.0_f32;
        let index = cube_face_at(yaw, pitch, egui::vec2(0.0, 0.0), reach).unwrap();
        let (normal, preset, _) = CUBE_FACES[index];
        assert_eq!(normal, [0, -1, 0], "the click landed on a face pointing away from the eye");
        assert_eq!(preset, ViewPreset::Front);
        // And a point outside the cube is not a face at all.
        assert_eq!(cube_face_at(yaw, pitch, egui::vec2(200.0, 0.0), reach), None);
    }

    #[test]
    fn every_cube_face_names_a_different_view() {
        let mut seen: Vec<&str> = Vec::new();
        for (normal, preset, label) in CUBE_FACES {
            assert!(!seen.contains(&label), "two faces are labelled {label}");
            seen.push(label);
            // The face you can see is the side the camera would be on.
            let (yaw, pitch) = preset.angles();
            let mut v = view();
            v.camera.yaw = yaw;
            v.camera.pitch = pitch;
            let eye = v.eye() - v.camera.target;
            let normal = Vec3::new(normal[0] as f64, normal[1] as f64, normal[2] as f64);
            assert!(
                eye.normalized().dot(normal) > 0.99,
                "clicking {label} would put the camera somewhere other than that face"
            );
        }
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
