//! Grabbing a manipulator handle and keeping hold of it.

use super::*;

// -- picking and grabbing in the viewport -------------------------------------

#[test]
pub(crate) fn clicking_a_shape_selects_it_when_nothing_is_selected_yet() {
    // Regression: picking lived in the manipulator, which returned early with no primary node, so the
    // first click into an empty selection did nothing.
    let mut harness = harness("pick-from-nothing");
    let plate = harness.state().primary().expect("the fixture puts a plate in");
    harness.state_mut().clear_selection();
    harness.step();
    assert!(harness.state().selection.is_empty());

    let at = harness.state().viewport_rect.center();
    press(&mut harness, at);
    release(&mut harness, at);

    assert_eq!(harness.state().selection, vec![plate], "clicking the shape did not select it");
}

#[test]
pub(crate) fn a_press_on_a_move_handle_grabs_it_even_though_the_pointer_leaves_it() {
    // Regression: a handle grabs within 9 px but egui calls it a drag only after about 6, so deciding
    // from the pointer's later position often missed. The press position decides.
    let mut harness = harness("grab-slip");
    let plate = harness.state().primary().unwrap();
    harness.state_mut().mode = crate::gizmo::Mode::Move;
    harness.step();

    let view = harness.state().current_view();
    let gizmo = harness.state().gizmo_for(plate).expect("a gizmo for the selected plate");
    let handle = crate::gizmo::Handle::MoveAxis(0);
    let (at, _) = view.project(gizmo.handle_point(handle, &view)).expect("the X arrow is off screen");

    // Past the grab radius on the very first move, the gesture the old code missed.
    let along = view.project(gizmo.handle_point(handle, &view) + gizmo.axes[0] * 30.0).unwrap().0;
    let steps = harness.state().history.undo_len();

    press(&mut harness, at);
    move_to(&mut harness, at + (along - at).normalized() * 12.0);
    assert!(harness.state().drag.is_some(), "the press on the arrow did not start a drag");
    move_to(&mut harness, along);
    release(&mut harness, along);

    let moved = harness.state().scene.node(plate).position;
    assert!(moved.x.abs() > 1e-9, "the drag on the X arrow moved the plate nowhere: {moved:?}");
    assert!(moved.y.abs() < 1e-9 && moved.z.abs() < 1e-9, "an X drag moved the other axes too: {moved:?}");
    assert_eq!(harness.state().history.undo_len(), steps + 1, "the drag left more than one thing to undo");
    assert_eq!(harness.state().selection, vec![plate], "grabbing a handle also re-picked what is behind it");
}
