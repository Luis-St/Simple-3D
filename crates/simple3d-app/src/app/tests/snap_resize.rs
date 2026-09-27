//! Snapping a resize, and what happens when snapping is not asked for.

use super::*;
use crate::gizmo::Handle;
use simple3d_core::config::SnapMode;
use simple3d_core::keymap::Command;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn a_face_resize_snaps_the_face_it_pulls_onto_another_body() {
    // Issue 68: a resize is a drag too, and pulling a face onto a neighbour should snap like sliding.
    let mut app = app_in(temp_config_dir("snap-resize"));
    let root = app.scene.root();
    let a = app.scene.add_primitive("box", root, 0).unwrap();
    let b = app.scene.add_primitive("box", root, 1).unwrap();
    // Half a millimetre off the grid, so only a snap can join the faces.
    app.scene.get_mut(b).unwrap().position = Vec3::new(43.5, 0.0, 0.0);
    app.select_only(a);
    app.run(Command::ModeResize);
    app.history.clear();
    app.reevaluate_for_test();

    // A corner on B's near face, since snaps take only visible features and that face is turned away.
    let (lo, hi) = app.evaluated.node_meshes[&b].bounds().unwrap();
    let face = Vec3::new(lo.x, lo.y, hi.z);

    app.settings.geometry_snap = SnapMode::Always;
    app.snap_requested = true;
    drag_gesture(&mut app, a, Handle::ResizeFace(0, true), face, 3);
    app.reevaluate_for_test();

    // A's +X face now sits exactly on B's -X face.
    let (_, hi) = app.evaluated.node_meshes[&a].bounds().unwrap();
    assert!((hi.x - lo.x).abs() < 1e-6, "the pulled face landed at {} rather than on {}", hi.x, lo.x);
    // 33.5 is unreachable on the 1 mm step, so only the snap got there.
    assert!((hi.x - 33.5).abs() < 1e-6, "{hi:?}");
}

#[test]
pub(crate) fn a_resize_without_snapping_asked_for_keeps_to_the_grid() {
    let mut app = app_in(temp_config_dir("snap-resize-off"));
    let root = app.scene.root();
    let a = app.scene.add_primitive("box", root, 0).unwrap();
    let b = app.scene.add_primitive("box", root, 1).unwrap();
    app.scene.get_mut(b).unwrap().position = Vec3::new(43.5, 0.0, 0.0);
    app.select_only(a);
    app.run(Command::ModeResize);
    app.history.clear();
    app.reevaluate_for_test();
    let (lo, _) = app.evaluated.node_meshes[&b].bounds().unwrap();

    app.settings.geometry_snap = SnapMode::Never;
    app.snap_requested = false;
    drag_gesture(&mut app, a, Handle::ResizeFace(0, true), Vec3::new(lo.x, 0.0, 0.0), 3);
    app.reevaluate_for_test();
    let (_, hi) = app.evaluated.node_meshes[&a].bounds().unwrap();
    assert!((hi.x - lo.x).abs() > 0.4, "the resize snapped to the face with snapping off: {hi:?}");
}

#[test]
pub(crate) fn a_snapped_drag_never_catches_a_feature_round_the_back_of_its_own_body() {
    // A drag must not catch far-side corners, whose projections land on the face in front.
    let mut app = app_in(temp_config_dir("snap-hidden"));
    let root = app.scene.root();
    let a = app.scene.add_primitive("box", root, 0).unwrap();
    let b = app.scene.add_primitive("box", root, 1).unwrap();
    app.scene.get_mut(b).unwrap().position = Vec3::new(43.0, 0.0, 0.0);
    app.select_only(a);
    app.history.clear();
    app.reevaluate_for_test();
    let view = app.current_view();

    // B's catchable features, and the ones the picture shows.
    let features = crate::snap::features_of(&app.evaluated.node_meshes[&b]);
    let hidden: Vec<Vec3> = features.iter().filter(|f| !app.in_clear_view(&view, f.point)).map(|f| f.point).collect();
    assert!(!hidden.is_empty(), "this view shows every corner of the box, so there is nothing to hide");

    // Pointing at a hidden corner catches something else or nothing, never it.
    for point in hidden {
        let cursor = view.project(point).unwrap().0;
        if let Some((caught, _)) = app.nearest_feature_excluding(&view, cursor, &[a]) {
            assert!(
                (caught.point - point).length() > 1e-6,
                "a drag caught {point:?}, which is round the back of the body"
            );
        }
    }
}

#[test]
pub(crate) fn a_move_drag_without_snapping_asked_for_keeps_to_the_grid() {
    // With snapping off the drag stays on the grid step.
    let mut app = app_in(temp_config_dir("snap-off"));
    let root = app.scene.root();
    let a = app.scene.add_primitive("box", root, 0).unwrap();
    let b = app.scene.add_primitive("box", root, 1).unwrap();
    app.scene.get_mut(b).unwrap().position = Vec3::new(43.0, 0.0, 0.0);
    app.select_only(a);
    app.history.clear();
    app.reevaluate_for_test();
    let (lo, _) = app.evaluated.node_meshes[&b].bounds().unwrap();
    let corner = Vec3::new(lo.x, lo.y, lo.z);

    app.settings.geometry_snap = SnapMode::Never;
    app.snap_requested = false;
    drag_gesture(&mut app, a, Handle::MoveAxis(0), corner, 3);
    assert!(
        (app.scene.node(a).position - corner).length() > 1.0,
        "the drag snapped to the corner with snapping off: {:?}",
        app.scene.node(a).position
    );
}
