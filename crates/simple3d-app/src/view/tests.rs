use super::cube::*;
use super::frame::*;
use super::preset::*;
use simple3d_core::scene::Camera;
use simple3d_geom::Vec3;

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
        assert!((closest - world).length() < 1e-3, "{world:?} -> {screen:?} -> off by {}", (closest - world).length());
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
            assert_eq!(cube_zone_at(yaw, pitch, at, reach), Some(*normal), "{label} at yaw {yaw} pitch {pitch}");
            let _ = index;
        }
    }
}

#[test]
fn a_face_turned_away_is_never_what_a_click_lands_on() {
    // Straight down the +Y axis: the back face is dead behind the front one,
    // and a click in the middle must be the front.
    let (yaw, pitch) = ViewPreset::Front.angles();
    let reach = 20.0_f32;
    let zone = cube_zone_at(yaw, pitch, egui::vec2(0.0, 0.0), reach).unwrap();
    assert_eq!(zone, [0, -1, 0], "the click landed on a face pointing away from the eye");
    assert_eq!(cube_zone_preset(zone), Some(ViewPreset::Front));
    // And a point outside the cube is not part of it at all.
    assert_eq!(cube_zone_at(yaw, pitch, egui::vec2(200.0, 0.0), reach), None);
}

#[test]
fn the_corners_and_edges_of_the_cube_can_be_pointed_at_too() {
    // Issue 34: a corner is how the three-quarter views are asked for, and
    // an edge is how the two beside it are.
    let reach = 20.0_f32;
    let (yaw, pitch) = ViewPreset::Isometric.angles();
    for zone in cube_zones() {
        let v = Vec3::new(zone[0] as f64, zone[1] as f64, zone[2] as f64);
        let (at, depth) = cube_project(yaw, pitch, v, reach);
        if depth >= 0.0 {
            continue;
        }
        assert_eq!(cube_zone_at(yaw, pitch, at, reach), Some(zone), "{zone:?} could not be pointed at");
    }
}

#[test]
fn a_corner_of_the_cube_asks_for_the_view_from_that_corner() {
    // The near corner of the isometric view is +X +Y up ... looking from
    // it means an eye 35 degrees above the ground, halfway between two
    // sides.
    let (yaw, pitch) = cube_zone_angles([1, 1, 1], 0.0);
    assert!((yaw - 45.0).abs() < 1e-6, "yaw was {yaw}");
    assert!((pitch - 35.264).abs() < 1e-3, "pitch was {pitch}");

    // An edge between two sides is halfway between them, and level.
    let (yaw, pitch) = cube_zone_angles([1, -1, 0], 0.0);
    assert!((yaw + 45.0).abs() < 1e-6, "yaw was {yaw}");
    assert_eq!(pitch, 0.0);

    // A face is still exactly the preset it always was, and the two poles
    // keep whatever yaw the camera already had -- turning the model round
    // on the way to looking straight down at it is motion for nothing.
    assert_eq!(cube_zone_angles([0, -1, 0], 17.0), ViewPreset::Front.angles());
    assert_eq!(cube_zone_angles([0, 0, 1], 17.0), (17.0, ViewPreset::Top.angles().1));
}

#[test]
fn every_zone_of_the_cube_is_named_after_the_sides_it_lies_between() {
    assert_eq!(cube_zone_label([0, -1, 0]), "Front");
    assert_eq!(cube_zone_label([1, -1, 0]), "right-front edge");
    assert_eq!(cube_zone_label([1, -1, 1]), "right-front-top corner");
    // Twenty-six of them, and no repeats.
    let mut labels: Vec<String> = cube_zones().iter().map(|zone| cube_zone_label(*zone)).collect();
    labels.sort();
    labels.dedup();
    assert_eq!(labels.len(), 26);
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
