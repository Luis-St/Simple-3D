//! The nudge step, the section plane, and the origin axes.

use super::*;
use simple3d_core::keymap::Command;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn the_step_governs_a_nudge_and_is_not_the_grid_spacing() {
    // The step is separate from the grid spacing (a 1 mm grid is unreadable) and governs nudges.
    let mut app = headless_app();
    let id = app.primary().unwrap();
    assert_eq!(app.move_snap(), 1.0, "the default step is one millimetre");

    app.scene.settings.snap_step = 2.5;
    app.scene.settings.grid_spacing = 10.0;
    let before = app.scene.node(id).position;
    app.run(Command::NudgeRight);
    let moved = app.scene.node(id).position - before;
    assert!((moved.length() - 2.5).abs() < 1e-9, "a nudge went {} rather than the 2.5 mm step", moved.length());
    assert_eq!(app.scene.settings.grid_spacing, 10.0, "the step changed the grid spacing with it");
}

#[test]
pub(crate) fn the_turn_step_governs_a_rotate_nudge_rather_than_a_fixed_fifteen_degrees() {
    // Issue 98: the turn step is settable and governs rotate nudges.
    let mut app = headless_app();
    let id = app.primary().unwrap();
    assert_eq!(app.settings.rotate_snap_deg, 15.0, "the default this test is about has changed");
    app.run(Command::ModeRotate);

    app.settings.rotate_snap_deg = 5.0;
    let before = app.scene.node(id).rotation;
    app.run(Command::NudgeRight);
    let turned = (app.scene.node(id).rotation - before).length();
    assert!((turned - 5.0).abs() < 1e-9, "a nudge turned {turned} degrees rather than the 5 degree step");

    // The set step, not the old fixed fifteen.
    assert!((turned - 15.0).abs() > 1e-9, "the nudge is still turning by the old fixed fifteen");
}

#[test]
pub(crate) fn the_section_plane_lands_in_the_middle_of_the_model_and_moves_without_an_edit() {
    // Issue 71: switched on, the plane starts mid-model, since at zero it may cut nothing.
    let mut app = headless_app();
    let id = app.primary().unwrap();
    app.scene.get_mut(id).unwrap().position = Vec3::new(40.0, 0.0, 0.0);
    app.reevaluate_for_test();
    let (lo, hi) = app.evaluated.mesh.bounds().expect("the plate is in the scene");
    // The plane starts on X, so that is the coordinate whose middle it finds.
    let middle = (lo.x + hi.x) / 2.0;

    assert!(!app.scene.settings.section.enabled, "a document starts whole");
    app.run(Command::ToggleSection);
    assert!(app.scene.settings.section.enabled);
    assert!(
        (app.scene.settings.section.offset - middle).abs() < 1e-9,
        "the plane opened at {} rather than the model's middle at {middle}",
        app.scene.settings.section.offset
    );

    // Moving it is not an edit, so undo has nothing to take back.
    let revision = app.history.revision();
    app.set_section_offset(middle + 5.0);
    assert_eq!(app.history.revision(), revision, "sliding the section wrote an undo step");
    assert!(!app.unsaved(), "sliding the section marked the document as changed");

    app.run(Command::ToggleSection);
    assert!(!app.scene.settings.section.enabled, "the switch does not switch off");
    assert!((app.scene.settings.section.offset - middle - 5.0).abs() < 1e-9, "the plane forgot where it stood");
}

#[test]
pub(crate) fn each_origin_axis_has_its_own_switch() {
    let mut app = headless_app();
    assert_eq!(app.scene.settings.axes_visible, [true; 3], "the axes start shown");
    app.run(Command::ToggleAxisY);
    assert_eq!(app.scene.settings.axes_visible, [true, false, true], "toggling Y touched another axis");
    app.run(Command::ToggleAxisY);
    assert_eq!(app.scene.settings.axes_visible, [true; 3]);
}

#[test]
fn a_section_kept_by_its_motion_keeps_what_lies_ahead_of_it() {
    // Motion mode: what the plane passed over goes, what lies ahead stays; pulled back, the reverse.
    let mut app = headless_app();
    app.scene.settings.section.keep = simple3d_core::scene::SectionKeep::Motion;
    app.run(Command::ToggleSection);
    let start = app.scene.settings.section.offset;
    let (ahead, behind) = (Vec3::new(start + 50.0, 0.0, 0.0), Vec3::new(start - 50.0, 0.0, 0.0));
    let cut = |app: &App| app.scene.settings.section.plane(app.evaluated.bounds, Vec3::new(0.0, 1.0, 0.0)).unwrap();

    app.set_section_offset(start + 1.0);
    assert!(cut(&app).keeps(ahead) && !cut(&app).keeps(behind), "slid along X, it kept what it had passed");
    // A slide that goes nowhere keeps the side.
    app.set_section_offset(start + 1.0);
    assert!(cut(&app).keeps(ahead), "standing still turned the cut round");

    app.set_section_offset(start - 1.0);
    assert!(!cut(&app).keeps(ahead) && cut(&app).keeps(behind), "slid back, it kept what it had passed");
}
