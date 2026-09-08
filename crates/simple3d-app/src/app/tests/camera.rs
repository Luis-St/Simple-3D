//! The view cube and the view menu reach the same camera.

use super::*;
use crate::view::CameraMove;
use simple3d_core::keymap::Command;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn the_view_cube_and_the_view_menu_reach_the_same_camera() {
    let mut app = headless_app();
    app.settings.reduce_motion = true;
    for (normal, preset, label) in crate::view::CUBE_FACES {
        app.run(Command::ViewIsometric);
        app.set_view(preset);
        // The angles may differ by a whole turn -- the camera takes the
        // short way round, so "left" can arrive at -180 rather than 180 --
        // so compare where the eye ends up, which is the thing that matters.
        let looking = app.current_view().offset_dir();
        let face = Vec3::new(normal[0] as f64, normal[1] as f64, normal[2] as f64);
        assert!(looking.dot(face) > 0.99, "{label}: the camera ended up at {looking:?}");
    }

    // With motion allowed the camera is on its way rather than already
    // there, and it gets there.
    app.settings.reduce_motion = false;
    app.run(Command::ViewIsometric);
    app.advance_camera();
    assert!(app.camera_move.is_some(), "the transition never started");
    let target = app.camera_move.unwrap().to;
    app.camera_move =
        Some(CameraMove { started: std::time::Instant::now() - crate::view::TRANSITION, ..app.camera_move.unwrap() });
    app.advance_camera();
    assert!(app.camera_move.is_none(), "the transition never finished");
    assert!((app.scene.camera.yaw - target.0).abs() < 1e-9);
}
