//! A fresh document, and one opened from the command line.

use super::*;
use simple3d_core::keymap::Command;
use simple3d_geom::Vec3;
use std::time::Duration;

#[test]
pub(crate) fn a_new_document_is_empty_and_unmodified() {
    // A shape nobody asked for is a shape they have to notice and delete,
    // and the palette is one click away. What matters as much: an untouched
    // new document has nothing to save, so quitting it asks no question.
    let mut app = app_in(temp_config_dir("empty-start"));
    assert_eq!(app.scene.depth_first(), vec![app.scene.root()], "something was added to the new document");
    assert!(app.primary().is_none());
    assert!(!app.unsaved(), "an untouched new document already counts as modified");

    // And File > New, from a document that does have something in it, gets
    // back to exactly that.
    let root = app.scene.root();
    app.scene.add_primitive("box", root, 0).unwrap();
    app.run(Command::New);
    assert_eq!(app.scene.depth_first(), vec![app.scene.root()]);
    assert!(!app.unsaved());
}

#[test]
pub(crate) fn a_message_fades_once_it_has_been_read_and_ready_never_does() {
    let message = Status::Info("Saved".into());
    assert_eq!(status_opacity(&message, Duration::from_secs(0)), 1.0);
    assert_eq!(status_opacity(&message, STATUS_LIFETIME), 1.0);
    assert!(status_opacity(&message, STATUS_LIFETIME + Duration::from_millis(500)) < 1.0);
    assert_eq!(status_opacity(&message, STATUS_LIFETIME + Duration::from_secs(2)), 0.0);
    // "Ready" is the state of the application, not news about it.
    assert_eq!(status_opacity(&Status::Idle, Duration::from_secs(600)), 1.0);
}

/// A project handed to the binary on the command line -- which is what a
/// file association and a double-click do -- keeps the camera it was saved
/// with. The starter scene has no camera worth keeping and is framed once
/// it has bounds; the flag is what tells the two apart, and it used to be
/// "this is the first evaluation of the session", which both are.
#[test]
pub(crate) fn a_project_opened_from_the_command_line_keeps_its_saved_camera() {
    let dir = temp_config_dir("cli-camera");
    let mut saver = app_in(dir.clone());
    let root = saver.scene.root();
    saver.scene.add_primitive("plate", root, 0).expect("the plate is in the registry");
    saver.scene.camera.target = Vec3::new(100.0, 100.0, 50.0);
    saver.scene.camera.distance = 250.0;
    saver.scene.camera.yaw = 33.0;
    saver.scene.camera.pitch = 12.0;
    let path = dir.join("camera.simple3d");
    saver.save_to(&path);
    let saved = saver.scene.camera;
    drop(saver);

    let ctx = egui::Context::default();
    let mut opened = App::with_config_dir(&ctx, Some(path.clone()), dir.clone());
    opened.viewport_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0));
    assert!(!opened.frame_when_evaluated, "opening a file from the command line asked for a reframe");
    // The reframe used to happen when the first evaluation landed, so the
    // camera has to still be the saved one *after* that.
    opened.reevaluate_for_test();
    assert_eq!(opened.scene.camera, saved, "the saved camera did not survive being opened from the command line");

    // And the empty starter document still frames itself, which is what the
    // flag exists for.
    let fresh = app_in(temp_config_dir("cli-camera-fresh"));
    assert!(fresh.frame_when_evaluated, "the starter scene will never be framed");
}
