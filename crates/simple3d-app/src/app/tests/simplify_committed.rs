//! The model's numbers while a simplify preview stands in a boolean (issue 106).

use super::simplify::{app_with_a_mesh, open_with};
use super::*;
use simple3d_core::scene::GroupOp;
use simple3d_geom::simplify::Simplify;
use std::time::{Duration, Instant};

/// Wait for the evaluation of the document as committed, as the frame loop would.
fn wait_for_committed(app: &mut App) {
    app.refresh_committed(true);
    let deadline = Instant::now() + Duration::from_secs(60);
    while !app.simplify_tool.as_ref().and_then(|tool| tool.committed.as_ref()).is_some_and(|c| c.stats.is_some()) {
        assert!(Instant::now() < deadline, "the committed evaluation never finished");
        std::thread::sleep(Duration::from_millis(2));
        app.refresh_committed(false);
    }
}

/// Regression: with the mesh cut by a box, the status bar's triangle count swapped the preview's own
/// triangles for the original's, which a difference does not keep one for one; and with nothing
/// selected, the scene's size was the preview's.
#[test]
fn the_status_bar_reads_the_committed_model_when_the_mesh_is_cut() {
    let (mut app, id) = app_with_a_mesh();
    let root = app.scene.root();
    let drilled = app.scene.add_group(GroupOp::Difference, root, 0);
    app.scene.reparent(id, drilled, 0).unwrap();
    let cutter = app.scene.add_primitive("box", drilled, 1).unwrap();
    app.scene.get_mut(cutter).unwrap().position = simple3d_geom::Vec3::new(12.0, 0.0, 0.0);
    app.reevaluate_for_test();
    app.selection.clear();
    let (size, count) = (app.selection_size_text(), app.evaluated.mesh.triangle_count());

    app.select_only(id);
    open_with(&mut app, Simplify { detail: 30, keep_sharp: false, ..Simplify::default() });
    app.selection.clear();
    assert_ne!(app.evaluated.mesh.triangle_count(), count, "no preview stands in the evaluation, so this proves nothing");
    wait_for_committed(&mut app);

    assert_eq!(app.committed_triangle_count(), count, "the status bar counts the preview");
    assert_eq!(app.selection_size_text(), size, "the status bar measures the preview");
}
