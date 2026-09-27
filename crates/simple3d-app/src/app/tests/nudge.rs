//! Nudging by key, and the undo steps it makes.

use super::*;
use simple3d_core::keymap::Command;
use simple3d_geom::Vec3;

/// Spec acceptance criterion 26 from the dispatched command: held repeats step by the snap and undo
/// in one. The arithmetic is covered in `gizmo`; this is the wiring from `App::run`.
#[test]
pub(crate) fn holding_an_arrow_key_nudges_by_the_snap_and_undoes_in_one_step() {
    const PRESSES: usize = 8;

    let mut app = headless_app();
    let id = app.primary().expect("the starter scene leaves a plate selected");
    let snap = app.move_snap();
    assert_eq!(snap, 1.0, "the default step changed; this test's arithmetic assumes it");
    let start = app.scene.node(id).position;

    // A step before the run, so "one step" differs from "stack emptied".
    app.run(Command::Rename);
    let before_run = app.history.revision();

    for _ in 0..PRESSES {
        app.run(Command::NudgeRight);
        app.reevaluate_for_test();
    }
    assert!(app.history.revision() > before_run, "the run recorded nothing at all");

    let travelled = (app.scene.node(id).position - start).length();
    assert!(
        (travelled - snap * PRESSES as f64).abs() < 1e-6,
        "{PRESSES} presses travelled {travelled} mm, expected {}",
        snap * PRESSES as f64
    );

    app.run(Command::Undo);
    assert_eq!(app.scene.node(id).position, start, "one undo did not restore the whole run");
    app.run(Command::Redo);
    assert!((app.scene.node(id).position - start).length() > snap, "redo did not put the run back");
}

/// Rotate and resize nudges use the same key, and a mode switch starts a new undo step.
#[test]
pub(crate) fn switching_mode_starts_a_new_nudge_undo_step() {
    let mut app = headless_app();
    let id = app.primary().unwrap();

    app.run(Command::ModeMove);
    app.run(Command::NudgeRight);
    app.reevaluate_for_test();
    let moved = app.scene.node(id).position;

    app.run(Command::ModeRotate);
    app.run(Command::NudgeRight);
    app.reevaluate_for_test();
    assert_ne!(app.scene.node(id).rotation, Vec3::ZERO, "a rotate-mode nudge did not rotate");

    // One undo takes back only the rotation.
    app.run(Command::Undo);
    assert_eq!(app.scene.node(id).rotation, Vec3::ZERO);
    assert_eq!(app.scene.node(id).position, moved, "the rotation and the move shared an undo step");
}

/// Spec acceptance criterion 20: a pasted copy stays selected, so a nudge or drag can follow.
#[test]
pub(crate) fn a_pasted_copy_is_left_selected() {
    let mut app = headless_app();
    let original = app.primary().unwrap();

    app.run(Command::Copy);
    app.run(Command::Paste);

    let pasted = app.primary().expect("nothing is selected after a paste");
    assert_ne!(pasted, original, "the paste left the original selected, not the copy");
    assert_eq!(app.selection, vec![pasted], "the copy is not the whole selection");
    assert_eq!(app.scene.node(pasted).position, app.scene.node(original).position);

    // A nudge acts on the copy straight away.
    app.reevaluate_for_test();
    let start = app.scene.node(pasted).position;
    app.run(Command::NudgeRight);
    assert_ne!(app.scene.node(pasted).position, start, "the pasted copy could not be nudged");
    assert_eq!(app.scene.node(original).position, start, "nudging the copy moved the original");
}
