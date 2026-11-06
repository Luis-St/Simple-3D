//! Running a function over a mesh (issue 81).

use super::simplify::app_with_a_mesh;
use super::*;
use simple3d_core::keymap::Command;
use simple3d_geom::shaping::{Function, Shaping};
use std::time::{Duration, Instant};

fn positions(app: &App, id: simple3d_core::scene::NodeId) -> Vec<simple3d_geom::Vec3> {
    app.scene.node(id).mesh().expect("it is a mesh").mesh.positions.clone()
}

/// Open the tool on the selection with `plan`, and wait for its run as the frame loop would.
fn open_with(app: &mut App, plan: Shaping) {
    app.run(Command::MeshFunction);
    app.shape_tool.as_mut().expect("the tool opened on the selection").plan = plan;
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        app.refresh_shape_tool();
        let tool = app.shape_tool.as_ref().expect("the tool is open");
        if tool.shown.as_ref().is_some_and(|shown| shown.plan == tool.plan) {
            app.reevaluate_for_test();
            return;
        }
        assert!(Instant::now() < deadline, "the function never finished");
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn noise() -> Shaping {
    Shaping { function: Function::Noise, refine: false, amount: 1.0, scale: 5.0, ..Shaping::default() }
}

#[test]
fn the_preview_stands_in_the_document_and_cancel_puts_the_mesh_back() {
    let (mut app, id) = app_with_a_mesh();
    let before = positions(&app, id);
    open_with(&mut app, noise());
    assert_ne!(positions(&app, id), before, "the document still holds the mesh it started with");
    assert_eq!(app.history.undo_len(), 0, "a preview took an undo step");
    // The panels read the mesh as it really is.
    assert_eq!(app.committed_mesh(id).expect("a mesh").mesh.positions, before);

    app.cancel_shape_tool();
    assert!(app.shape_tool.is_none());
    assert_eq!(positions(&app, id), before, "the mesh did not come back");
    assert_eq!(app.history.undo_len(), 0);
}

#[test]
fn applying_is_one_undo_step_back_to_the_original() {
    let (mut app, id) = app_with_a_mesh();
    let before = positions(&app, id);
    open_with(&mut app, noise());
    let shaped = positions(&app, id);

    app.apply_shape();
    assert!(app.shape_tool.is_none());
    assert_eq!(positions(&app, id), shaped);
    assert_eq!(app.history.undo_len(), 1);
    assert_eq!(app.settings.last_shaping, noise(), "the settings were not remembered for next time");

    app.run(Command::Undo);
    assert_eq!(positions(&app, id), before, "undo did not return to the original");
}

#[test]
fn an_undo_step_taken_while_previewing_does_not_hold_the_preview() {
    let (mut app, id) = app_with_a_mesh();
    let before = positions(&app, id);
    open_with(&mut app, noise());
    app.edit("Something else", None);
    app.cancel_shape_tool();
    app.run(Command::Undo);
    assert_eq!(positions(&app, id), before, "the snapshot held an unaccepted result");
}

#[test]
fn the_two_mesh_tools_never_preview_at_once() {
    let (mut app, id) = app_with_a_mesh();
    let before = positions(&app, id);
    open_with(&mut app, noise());
    app.run(Command::SimplifyMesh);
    assert!(app.shape_tool.is_none(), "opening Simplify left the function's preview open");
    assert!(app.simplify_tool.is_some());
    assert_eq!(positions(&app, id), before, "the function's preview was left in the document");
}

#[test]
fn only_a_mesh_opens_the_tool() {
    let mut app = app_in(temp_config_dir("shape"));
    let root = app.scene.root();
    let id = app.scene.add_primitive("box", root, 0).expect("a box");
    app.select_only(id);
    app.run(Command::MeshFunction);
    assert!(app.shape_tool.is_none());
}
