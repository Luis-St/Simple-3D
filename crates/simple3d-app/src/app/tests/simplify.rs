//! Dropping detail from a mesh (issue 106).

use super::*;
use simple3d_core::keymap::Command;
use simple3d_geom::simplify::Simplify;
use std::time::{Duration, Instant};

/// A document with one sphere baked into a mesh: thousands of triangles, no flat faces or creases.
fn app_with_a_mesh() -> (App, simple3d_core::scene::NodeId) {
    let mut app = app_in(temp_config_dir("simplify"));
    let root = app.scene.root();
    let id = app.scene.add_primitive("sphere", root, 0).expect("the sphere is in the registry");
    app.select_only(id);
    app.reevaluate_for_test();
    app.run(Command::ConvertToMesh);
    app.reevaluate_for_test();
    app.history.clear();
    app.saved_revision = app.history.revision();
    (app, id)
}

fn triangles(app: &App, id: simple3d_core::scene::NodeId) -> usize {
    app.scene.node(id).mesh().expect("it is a mesh").triangle_count()
}

/// Wait for the tool's background run, as the frame loop would.
fn wait_for_preview(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        app.refresh_simplify_tool();
        let tool = app.simplify_tool.as_ref().expect("the tool is open");
        if tool.shown.as_ref().is_some_and(|shown| shown.plan == tool.plan) {
            app.reevaluate_for_test();
            return;
        }
        assert!(Instant::now() < deadline, "the simplification never finished");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Open the tool on the selection and set its numbers.
fn open_with(app: &mut App, plan: Simplify) {
    app.run(Command::SimplifyMesh);
    app.simplify_tool.as_mut().expect("the tool opened on the selection").plan = plan;
    wait_for_preview(app);
}

#[test]
fn the_preview_is_the_result_and_stands_in_the_document() {
    // The preview is the simplified mesh itself, in the document without an undo step.
    let (mut app, id) = app_with_a_mesh();
    let before = triangles(&app, id);
    let steps = app.history.undo_len();

    open_with(&mut app, Simplify { detail: 30, keep_sharp: false, ..Simplify::default() });

    assert!(triangles(&app, id) < before, "the document still holds the mesh it started with");
    assert_eq!(app.history.undo_len(), steps, "a preview took an undo step");
}

#[test]
fn cancelling_puts_the_mesh_back() {
    let (mut app, id) = app_with_a_mesh();
    let before = triangles(&app, id);
    open_with(&mut app, Simplify { detail: 30, keep_sharp: false, ..Simplify::default() });
    assert!(triangles(&app, id) < before);

    app.cancel_simplify_tool();
    assert!(app.simplify_tool.is_none());
    assert_eq!(triangles(&app, id), before, "the mesh did not come back");
    assert_eq!(app.history.undo_len(), 0, "cancelling left an undo step behind");
}

#[test]
fn keeping_the_result_is_one_undo_step_back_to_the_original() {
    let (mut app, id) = app_with_a_mesh();
    let before = triangles(&app, id);
    open_with(&mut app, Simplify { detail: 30, keep_sharp: false, ..Simplify::default() });
    let previewed = triangles(&app, id);

    app.apply_simplify();
    assert!(app.simplify_tool.is_none(), "the window stayed open");
    assert_eq!(triangles(&app, id), previewed, "what was kept is not what was shown");
    assert_eq!(app.history.undo_len(), 1, "keeping the result should be one step");

    // Undo returns to the original mesh, not another preview.
    app.run(Command::Undo);
    assert_eq!(triangles(&app, id), before, "undo did not bring the detail back");
}

/// Every run starts from the mesh the tool opened on, not the preview, so the percentage always
/// means the same thing.
#[test]
fn turning_the_detail_back_up_recovers_it() {
    let (mut app, id) = app_with_a_mesh();
    let before = triangles(&app, id);
    open_with(&mut app, Simplify { detail: 20, keep_sharp: false, ..Simplify::default() });
    let coarse = triangles(&app, id);

    app.simplify_tool.as_mut().unwrap().plan.detail = 80;
    wait_for_preview(&mut app);
    let finer = triangles(&app, id);
    assert!(finer > coarse * 2, "80% of the mesh came back as {finer} against {coarse} at 20%");
    assert!(finer < before, "nothing was dropped at all");
}

/// A shape where every edge is a corner cannot be reduced, and the tool says so.
#[test]
fn features_the_settings_keep_can_refuse_the_whole_budget() {
    let mut app = app_in(temp_config_dir("simplify-box"));
    let root = app.scene.root();
    let id = app.scene.add_primitive("box", root, 0).expect("the box is in the registry");
    app.select_only(id);
    app.reevaluate_for_test();
    app.run(Command::ConvertToMesh);
    app.reevaluate_for_test();
    let before = triangles(&app, id);

    open_with(&mut app, Simplify { detail: 10, ..Simplify::default() });
    assert_eq!(triangles(&app, id), before, "a box lost triangles with its creases being kept");
}

#[test]
fn only_a_mesh_can_be_simplified() {
    let mut app = headless_app();
    app.run(Command::SimplifyMesh);
    assert!(app.simplify_tool.is_none(), "the tool opened on a primitive");
    assert!(app.status_text().contains("convert"), "the refusal does not say what to do: {}", app.status_text());
}

/// Leaving the document removes the preview, or the tab returns holding an unaccepted result.
#[test]
fn leaving_the_tab_puts_the_mesh_back() {
    let (mut app, id) = app_with_a_mesh();
    let before = triangles(&app, id);
    open_with(&mut app, Simplify { detail: 30, keep_sharp: false, ..Simplify::default() });
    assert!(triangles(&app, id) < before);

    app.new_project();
    app.activate_tab(0);
    assert!(app.simplify_tool.is_none(), "the tool followed the document it was not about");
    assert_eq!(triangles(&app, id), before, "the mesh came back simplified");
}

/// A run in flight keeps requesting frames: its channel answer is not a toolkit event, so a
/// sleeping loop never showed the preview.
#[test]
fn a_run_in_flight_keeps_the_frames_coming() {
    let (mut app, _) = app_with_a_mesh();
    app.run(Command::SimplifyMesh);
    app.simplify_tool.as_mut().expect("the tool is open").plan.detail = 30;
    app.refresh_simplify_tool();
    assert!(app.simplify_tool.as_ref().unwrap().job.is_some(), "no run was started for the new number");
    assert!(app.work_in_flight(), "the loop would go to sleep with the preview still to come");
}

/// An edit while the window is open does not snapshot the preview, or undo would step back to an
/// unaccepted simplification with no way back.
#[test]
fn an_edit_made_while_the_window_is_open_does_not_record_the_preview() {
    let (mut app, id) = app_with_a_mesh();
    let before = triangles(&app, id);
    open_with(&mut app, Simplify { detail: 30, keep_sharp: false, ..Simplify::default() });
    assert!(triangles(&app, id) < before, "there is no preview standing to be recorded");

    app.edit("Rename", None);
    app.scene.get_mut(id).expect("it is there").name = "Renamed".into();
    assert!(triangles(&app, id) < before, "the preview was not put back after the snapshot");

    app.run(Command::Undo);
    assert_eq!(triangles(&app, id), before, "undo stepped back to the preview rather than to the mesh");
}

/// A save does not write the preview.
#[test]
fn saving_while_the_window_is_open_writes_the_mesh_rather_than_the_preview() {
    let (mut app, id) = app_with_a_mesh();
    let before = triangles(&app, id);
    open_with(&mut app, Simplify { detail: 30, keep_sharp: false, ..Simplify::default() });

    let path = temp_config_dir("simplify-save").join("model.simple3d");
    app.save_to(&path);
    let written = simple3d_core::project::from_str(&std::fs::read_to_string(&path).expect("it was written"))
        .expect("it reads back");
    let saved = written
        .depth_first()
        .into_iter()
        .find_map(|node| written.node(node).mesh().map(|mesh| mesh.triangle_count()))
        .expect("the mesh is in the file");
    assert_eq!(saved, before, "the file holds the preview rather than the mesh");
    assert!(triangles(&app, id) < before, "the preview was not put back after the save");
}

/// Frames keep coming after the result lands: it only marks the scene dirty and the shape shows
/// two frames later, which nothing else requests.
#[test]
fn a_landed_result_still_asks_for_the_frame_that_evaluates_it() {
    let (mut app, _) = app_with_a_mesh();
    app.run(Command::SimplifyMesh);
    app.simplify_tool.as_mut().expect("the tool is open").plan.detail = 30;
    // As at the top of a frame: anything owed to the evaluator has just been handed over.
    app.dirty = false;
    let deadline = Instant::now() + Duration::from_secs(60);
    while app.simplify_tool.as_ref().is_some_and(|tool| tool.shown.is_none()) {
        app.refresh_simplify_tool();
        assert!(Instant::now() < deadline, "the simplification never finished");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(app.dirty, "the result was put in the document without marking it to be evaluated");
    assert!(app.work_in_flight(), "the loop would sleep with the new shape never drawn");
}

/// Toggling the triangle overlay must be part of the viewport's image cache key, or the
/// wireframe stayed until something else redrew.
#[test]
fn turning_the_triangles_off_redraws_the_viewport() {
    let (mut app, _) = app_with_a_mesh();
    open_with(&mut app, Simplify { detail: 30, keep_sharp: false, ..Simplify::default() });
    let size = [800, 600];
    let before = crate::panel_viewport::image_key(&app, size, true);

    app.simplify_tool.as_mut().unwrap().wireframe = false;
    assert_ne!(crate::panel_viewport::image_key(&app, size, true), before, "the picture would not be drawn again");
}

/// The window draws with every field and the summary on a real frame.
#[test]
fn the_window_draws() {
    let (mut app, _) = app_with_a_mesh();
    app.run(Command::SimplifyMesh);
    draw_one_frame(&mut app);
    assert!(app.simplify_tool.is_some(), "drawing the window closed it");
}
