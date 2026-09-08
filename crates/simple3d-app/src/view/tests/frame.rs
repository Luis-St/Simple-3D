//! Framing bounds, and moving between viewpoints.

use super::*;
use simple3d_core::scene::Camera;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn framing_bounds_centres_and_fits_them() {
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
pub(crate) fn a_turn_takes_the_short_way_round() {
    assert_eq!(shortest_turn(170.0, -170.0), 20.0);
    assert_eq!(shortest_turn(-170.0, 170.0), -20.0);
    assert_eq!(shortest_turn(0.0, 90.0), 90.0);
    assert_eq!(shortest_turn(0.0, 180.0), 180.0);
    assert_eq!(shortest_turn(10.0, 10.0), 0.0);
    // However many times the camera has been orbited round.
    assert_eq!(shortest_turn(720.0 + 10.0, 20.0), 10.0);
}

#[test]
pub(crate) fn a_transition_starts_where_it_was_and_ends_where_it_was_asked_for() {
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
pub(crate) fn every_view_preset_has_distinct_angles() {
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
pub(crate) fn the_top_preset_looks_down() {
    let mut v = view();
    let (yaw, pitch) = ViewPreset::Top.angles();
    v.camera.yaw = yaw;
    v.camera.pitch = pitch;
    assert!(v.forward().z < -0.99, "{:?}", v.forward());
    assert!(v.eye().z > 90.0);
}
