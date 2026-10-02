//! A manipulator drag on one of several selected nodes carries the others along.

use super::*;
use crate::gizmo::{self, Handle, Mode};
use simple3d_core::keymap::Command;
use simple3d_core::scene::NodeId;
use simple3d_geom::Vec3;

/// Two boxes, `a` at the origin and `b` beside it, both selected with `b` last.
fn both_selected(tag: &str, mode: Mode) -> (App, NodeId, NodeId) {
    let (mut app, a, b) = two_boxes(tag, Vec3::new(40.0, 0.0, 0.0));
    app.toggle_selected(b);
    assert_eq!(app.primary(), Some(b));
    app.mode = mode;
    app.reevaluate_for_test();
    (app, a, b)
}

fn extent(app: &App, id: NodeId, axis: usize) -> f64 {
    let (lo, hi) = app.evaluated.node_world_bounds[&id];
    hi.get(axis) - lo.get(axis)
}

/// Regression: a move handle moved only the last selected node, though a Transform field applies to
/// all of them. One undo takes back both.
#[test]
pub(crate) fn a_move_drag_carries_the_whole_selection_and_undoes_in_one_step() {
    let (mut app, a, b) = both_selected("carry-move", Mode::Move);
    let (a0, b0) = (app.scene.node(a).position, app.scene.node(b).position);
    let origin = app.gizmo_for(b).unwrap().origin;
    drag_gesture(&mut app, b, Handle::MoveAxis(1), origin + Vec3::new(0.0, 30.0, 0.0), 5);
    let moved = app.scene.node(b).position - b0;
    assert!(moved.length() > 1.0, "the drag did not move the dragged box");
    assert!((app.scene.node(a).position - a0 - moved).length() < 1e-9, "the other box was not carried the same way");

    app.run(Command::Undo);
    assert_eq!((app.scene.node(a).position, app.scene.node(b).position), (a0, b0), "one undo missed a box");
}

/// Escape restores every carried node, not only the dragged one.
#[test]
pub(crate) fn a_cancelled_drag_restores_the_whole_selection() {
    let (mut app, a, b) = both_selected("carry-cancel", Mode::Move);
    let a0 = app.scene.node(a).position;
    let handle = Handle::MoveAxis(0);
    let gizmo = app.gizmo_for(b).unwrap();
    let view = app.current_view();
    let from = view.project(gizmo.handle_point(handle, &view)).unwrap().0;
    let to = view.project(gizmo.origin + gizmo.axes[0] * 30.0).unwrap().0;
    app.manipulate_step(&gizmo, &view, b, gizmo::DragPhase::Begin, Some(handle), Some(from), Default::default());
    app.manipulate_step(&gizmo, &view, b, gizmo::DragPhase::Continue, Some(handle), Some(to), Default::default());
    assert_ne!(app.scene.node(a).position, a0, "the other box was never carried, so cancelling proves nothing");
    app.manipulate_step(&gizmo, &view, b, gizmo::DragPhase::Cancel, Some(handle), Some(to), Default::default());
    assert_eq!(app.scene.node(a).position, a0);
}

/// A turn orbits the other nodes about the same pivot and turns them by the same angle.
#[test]
pub(crate) fn a_rotate_drag_turns_the_whole_selection_about_the_pivot() {
    let (mut app, a, b) = both_selected("carry-rotate", Mode::Rotate);
    let pivot = app.gizmo_for(b).unwrap().origin;
    let a0 = app.scene.node(a).position;
    drag_gesture(&mut app, b, Handle::RotateRing(2), pivot + Vec3::new(20.0, 25.0, 0.0), 4);
    let turned = app.scene.node(b).rotation.z;
    assert!(turned.abs() > 1.0, "the drag did not turn the dragged box");
    assert!((app.scene.node(a).rotation.z - turned).abs() < 1e-9, "the other box did not turn by the same angle");
    let a1 = app.scene.node(a).position;
    assert!(((a1 - pivot).length() - (a0 - pivot).length()).abs() < 1e-6, "the other box left its orbit");
    assert!((a1 - a0).length() > 1.0, "the other box turned in place instead of about the pivot");
}

/// Scale gives every node the same factor along the pulled axis.
#[test]
pub(crate) fn a_scale_drag_scales_the_whole_selection_by_the_same_factor() {
    let (mut app, a, b) = both_selected("carry-scale", Mode::Scale);
    let gizmo = app.gizmo_for(b).unwrap();
    let face = gizmo.handle_point(Handle::ResizeFace(0, true), &app.current_view());
    drag_gesture(&mut app, b, Handle::ResizeFace(0, true), face + Vec3::new(10.0, 0.0, 0.0), 3);
    let factor = app.scene.node(b).scale.x;
    assert!((factor - 1.0).abs() > 0.05, "the drag did not scale the dragged box");
    assert!((app.scene.node(a).scale.x - factor).abs() < 1e-9, "the other box was not scaled by the same factor");
}

/// Resize grows every node by the same millimetres along the pulled axis.
#[test]
pub(crate) fn a_resize_drag_grows_the_whole_selection_by_the_same_length() {
    let (mut app, a, b) = both_selected("carry-resize", Mode::Resize);
    let (wa, wb) = (extent(&app, a, 0), extent(&app, b, 0));
    let gizmo = app.gizmo_for(b).unwrap();
    let face = gizmo.handle_point(Handle::ResizeFace(0, true), &app.current_view());
    drag_gesture(&mut app, b, Handle::ResizeFace(0, true), face + Vec3::new(10.0, 0.0, 0.0), 3);
    app.reevaluate_for_test();
    let grown = extent(&app, b, 0) - wb;
    assert!(grown.abs() > 1.0, "the drag did not resize the dragged box");
    assert!((extent(&app, a, 0) - wa - grown).abs() < 1e-6, "the other box did not grow by the same length");
}
