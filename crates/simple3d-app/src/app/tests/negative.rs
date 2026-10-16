//! Negative parts (issue 119) and primitive shortcuts (issue 124) through the app.

use super::*;
use crate::app::{CSG_DIFFERENCE, CSG_UNION};
use simple3d_core::keymap::{Action, Command};
use simple3d_core::scene::GroupOp;

#[test]
pub(crate) fn toggling_a_negative_part_is_one_step_draws_it_as_a_ghost_and_never_marks_the_root() {
    let mut app = app_in(temp_config_dir("negative-toggle"));
    let root = app.scene.root();
    let plate = app.scene.add_primitive("plate", root, 0).unwrap();
    let hole = app.scene.add_primitive("cylinder", root, 1).unwrap();
    app.select_only(hole);
    app.run(Command::ToggleNegative);
    assert!(app.scene.node(hole).negative);
    assert!(app.ghosts().is_empty(), "a negative part was drawn while the document hides cutters");
    app.scene.settings.show_cutters = true;
    assert_eq!(app.ghosts(), vec![hole], "the document shows cutters, so the negative part is a ghost");
    app.scene.get_mut(hole).unwrap().visible = false;
    assert!(app.ghosts().is_empty(), "a hidden negative part is not drawn");
    app.scene.get_mut(hole).unwrap().visible = true;
    app.scene.settings.show_cutters = false;

    app.undo();
    assert!(!app.scene.node(hole).negative);

    app.set_negative(&[root, plate], true);
    assert!(!app.scene.node(root).negative, "the root became a negative part");
    assert!(app.scene.node(plate).negative);
}

#[test]
pub(crate) fn showing_cutters_draws_a_differences_cutters_but_never_its_base() {
    let mut app = app_in(temp_config_dir("negative-cutters"));
    let root = app.scene.root();
    let group = app.scene.add_group(GroupOp::Difference, root, 0);
    let base = app.scene.add_primitive("plate", group, 0).unwrap();
    let cutter = app.scene.add_primitive("cylinder", group, 1).unwrap();
    assert!(app.ghosts().is_empty(), "cutters are drawn by default");
    app.scene.settings.show_cutters = true;
    assert_eq!(app.ghosts(), vec![cutter]);
    // A hidden base hands the role to the next child, which is then no cutter.
    app.scene.get_mut(base).unwrap().visible = false;
    assert!(app.ghosts().is_empty(), "the new base was drawn as a cutter");
}

#[test]
pub(crate) fn a_dragged_body_is_cut_by_its_negative_siblings_on_the_card() {
    let mut app = app_in(temp_config_dir("negative-csg"));
    let root = app.scene.root();
    let group = app.scene.add_group(GroupOp::Union, root, 0);
    let hole = app.scene.add_primitive("cylinder", group, 0).unwrap();
    app.scene.get_mut(hole).unwrap().negative = true;
    let plate = app.scene.add_primitive("plate", group, 1).unwrap();
    let boss = app.scene.add_primitive("box", group, 2).unwrap();
    app.reevaluate_for_test();
    app.scene_renderable = crate::render::Renderable::prepare_scene(&app.evaluated.mesh, &app.evaluated.ranges);

    let (_, _, program) = app.csg_plan(boss).expect("a union with a negative part is drawable");
    // The solids first, unioned, then the negative part cut from them, whatever the tree order.
    assert_eq!(program, vec![0, 1, CSG_UNION, 2, CSG_DIFFERENCE]);
    let _ = plate;
}

#[test]
pub(crate) fn a_primitive_shortcut_recorded_on_its_tile_adds_the_shape() {
    let mut app = headless_app();
    assert_eq!(app.keymap.holder(&simple3d_core::keymap::Chord::key("F8"), None), None, "pick a free key");
    app.start_recording(Action::Primitive("cylinder".into()));
    let down = egui::Event::Key {
        key: egui::Key::F8,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    // Recorded in the main window, with no dialog open.
    shortcut_frame(&mut app, vec![down.clone()]);
    shortcut_frame(&mut app, Vec::new());
    assert_eq!(app.recording, None, "the release should have finished the recording");
    assert_eq!(app.keymap.primitive_shortcut_text("cylinder"), "F8");

    let cylinders = |app: &App| {
        app.scene.ids().filter(|&id| app.scene.node(id).spec().is_some_and(|s| s.type_id == "cylinder")).count()
    };
    let before = cylinders(&app);
    shortcut_frame(&mut app, vec![down]);
    assert_eq!(cylinders(&app), before + 1, "the shortcut did not add a cylinder");
}

/// One frame of the main window's key handling, which `draw_frame_with` leaves to the run loop.
fn shortcut_frame(app: &mut App, events: Vec<egui::Event>) {
    let ctx = egui::Context::default();
    let input = egui::RawInput { events, ..Default::default() };
    let _ = ctx.run(input, |ctx| app.handle_shortcuts(ctx));
}

#[test]
pub(crate) fn a_negative_part_on_the_move_is_a_moving_cutter_and_not_a_ghost() {
    let mut app = app_in(temp_config_dir("negative-moving"));
    let root = app.scene.root();
    let plate = app.scene.add_primitive("plate", root, 0).unwrap();
    let hole = app.scene.add_primitive("cylinder", root, 1).unwrap();
    app.scene.get_mut(hole).unwrap().negative = true;
    app.scene.settings.show_cutters = true;
    app.reevaluate_for_test();
    app.settling = Some(hole);
    let moving: Vec<_> = app.moving_cutters().into_iter().map(|(id, _)| id).collect();
    assert_eq!(moving, vec![hole], "the moving negative part is not drawn as its red shell");
    assert!(app.ghosts().is_empty(), "the moving negative part was drawn as a ghost as well");
    app.settling = Some(plate);
    assert!(app.moving_cutters().is_empty(), "a moving solid was drawn as a cutter");
}
