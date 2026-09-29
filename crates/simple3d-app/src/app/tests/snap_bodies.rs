//! Snapping and measuring see the model as drawn, not the shapes a boolean is made of.

use super::*;
use simple3d_core::scene::{GroupOp, NodeId};
use simple3d_geom::Vec3;

/// A dodecahedron with its top half cut away by a box, as reported: the cut outline slants, and the
/// box's own corners and faces are nowhere in the picture.
fn cut_dodecahedron() -> (App, NodeId, NodeId) {
    let mut app = headless_app();
    let root = app.scene.root();
    for id in app.scene.descendants(root) {
        app.scene.remove(id);
    }
    let group = app.scene.add_group(GroupOp::Difference, root, 0);
    app.scene.add_primitive("dodecahedron", group, 0).unwrap();
    let cutter = app.scene.add_primitive("box", group, 1).unwrap();
    let node = app.scene.get_mut(cutter).unwrap();
    node.position = Vec3::new(0.0, 0.0, 25.0);
    node.scale = Vec3::new(3.0, 3.0, 2.5);
    app.scene.camera.pitch = 30.0;
    app.reevaluate_for_test();
    (app, group, cutter)
}

#[test]
pub(crate) fn a_measure_click_catches_only_what_is_left_of_a_cut_shape() {
    // Regression: the uncut dodecahedron and the cutter were snapped to, so a pointer in thin air
    // caught the cut-away top corner, and one on the cut caught a face reaching above it.
    let (app, _, _) = cut_dodecahedron();
    let view = crate::view::View::new(app.scene.camera, app.viewport_rect);
    let top = Vec3::new(-3.5682208977308996, 0.0, 9.341723589627156);
    let (screen, _) = view.project(top).unwrap();
    let caught = app.measure_catch(&view, screen).map(|(point, _)| point);
    assert!(caught.is_none_or(|point| point.kind.is_none()), "the cut-away corner was caught: {caught:?}");

    // Every feature measuring can catch is on what is left: nothing above the cut, nothing of the box.
    for (id, mesh) in app.snap_bodies(&[]) {
        for feature in app.snaps_of(id, &mesh).features.iter() {
            assert!(feature.point.z < 1e-6, "{:?} at {:?} is above the cut", feature.kind, feature.point);
            assert!(feature.point.x.abs() < 10.0, "{:?} at {:?} is the cutter's", feature.kind, feature.point);
        }
    }
}

#[test]
pub(crate) fn a_boolean_is_one_body_unless_one_of_its_parts_is_dragged() {
    let (app, group, cutter) = cut_dodecahedron();
    let bodies: Vec<NodeId> = app.snap_bodies(&[]).into_iter().map(|(id, _)| id).collect();
    assert_eq!(bodies, vec![group], "the difference is not one body");
    // Dragging the cutter, its group is taken apart so the cutter can be left out and the rest caught.
    let dragging: Vec<NodeId> = app.snap_bodies(&[cutter]).into_iter().map(|(id, _)| id).collect();
    assert_eq!(dragging.len(), 1, "{dragging:?}");
    assert_ne!(dragging[0], cutter, "the dragged cutter is still a target");
    assert_ne!(dragging[0], group, "the group holding the dragged cutter is still whole");
    // A dragged group carries its result, not its parts.
    assert_eq!(app.bodies_of(group).len(), 1);
}
