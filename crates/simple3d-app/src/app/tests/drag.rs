//! A manipulator drag over however many frames it takes.

use super::*;
use crate::gizmo::{self, Handle};
use simple3d_core::keymap::Command;
use simple3d_core::scene::NodeId;
use simple3d_geom::Vec3;

/// Drive a whole manipulator gesture as `panel_viewport::manipulate` does: `Begin`, `frames`
/// times `Continue`, then `Finish`.
pub(crate) fn drag_gesture(app: &mut App, id: NodeId, handle: Handle, to: Vec3, frames: usize) {
    let view = app.current_view();
    let start_gizmo = app.gizmo_for(id).expect("a gizmo for the dragged node");
    let from = view.project(start_gizmo.handle_point(handle, &view)).unwrap().0;
    let target = view.project(to).unwrap().0;

    app.hover_handle = Some(handle);
    let pointer = gizmo::PointerState { started: true, on_handle: true, have_cursor: true, ..Default::default() };
    let phase = gizmo::drag_phase(false, pointer);
    assert_eq!(phase, gizmo::DragPhase::Begin);
    app.manipulate_step(&start_gizmo, &view, id, phase, Some(handle), Some(from), gizmo::Mods::default());

    for frame in 1..=frames {
        let t = frame as f32 / frames as f32;
        let at = from + (target - from) * t;
        let pointer = gizmo::PointerState { have_cursor: true, ..Default::default() };
        let phase = gizmo::drag_phase(true, pointer);
        assert_eq!(phase, gizmo::DragPhase::Continue);
        // Rebuilt every frame, as `panel_viewport::manipulate` does; see `gizmo::Drag::gizmo`.
        let live = app.gizmo_for(id).expect("a gizmo mid-drag");
        app.manipulate_step(&live, &view, id, phase, Some(handle), Some(at), gizmo::Mods::default());
    }

    let live = app.gizmo_for(id).expect("a gizmo at the end of the drag");
    let pointer = gizmo::PointerState { released: true, have_cursor: true, ..Default::default() };
    let phase = gizmo::drag_phase(true, pointer);
    assert_eq!(phase, gizmo::DragPhase::Finish);
    app.manipulate_step(&live, &view, id, phase, Some(handle), Some(target), gizmo::Mods::default());
}

/// A drag lands where the cursor left it, the same for odd and even frame counts. Regression:
/// `Drag::update` measured against the live gizmo, which moved with the node, so the position
/// flipped every other frame.
#[test]
pub(crate) fn a_drag_lands_in_the_same_place_however_many_frames_it_took() {
    // Aim points taken from the starting scene.
    let (target, corner) = {
        let app = headless_app();
        let id = app.primary().unwrap();
        let gizmo = app.gizmo_for(id).unwrap();
        (gizmo.origin + Vec3::new(30.0, 0.0, 0.0), gizmo.origin + Vec3::new(25.0, 15.0, 0.0))
    };

    // Every handle kind, since all measure against the frozen frame now.
    for (handle, to) in [
        (Handle::MoveAxis(0), target),
        (Handle::MovePlane(2), corner),
        (Handle::RotateRing(2), corner),
        (Handle::ResizeFace(0, true), target),
        (Handle::ResizeCorner([true, true, true]), corner),
    ] {
        let mut landed = Vec::new();
        for frames in [1usize, 2, 3, 4, 5, 20, 21] {
            let mut app = headless_app();
            let id = app.primary().unwrap();
            drag_gesture(&mut app, id, handle, to, frames);
            app.reevaluate_for_test();
            let node = app.scene.node(id);
            landed.push((frames, node.position, node.rotation, node.params().cloned()));
        }
        let first = &landed[0];
        for entry in &landed {
            assert_eq!(
                (entry.1, entry.2, &entry.3),
                (first.1, first.2, &first.3),
                "{handle:?}: a {}-frame drag landed somewhere a {}-frame drag did not",
                entry.0,
                first.0
            );
        }
    }
}

/// Spec acceptance criterion 23: a twenty-frame drag leaves exactly one undo step.
#[test]
pub(crate) fn a_completed_drag_undoes_in_one_step_however_many_frames_it_took() {
    let mut app = headless_app();
    let id = app.primary().unwrap();
    let start = app.scene.node(id).position;

    // A recorded edit first, so "one step" is not "the stack emptied".
    app.edit("Before", None);
    let before = app.history.revision();

    let handle = Handle::MoveAxis(0);
    let origin = app.gizmo_for(id).unwrap().origin;
    drag_gesture(&mut app, id, handle, origin + Vec3::new(30.0, 0.0, 0.0), 20);
    app.reevaluate_for_test();

    let moved = app.scene.node(id).position;
    assert_ne!(moved, start, "the drag did not move anything");
    assert!(app.history.revision() > before);

    app.run(Command::Undo);
    assert_eq!(app.scene.node(id).position, start, "one undo did not take back the whole drag");
    assert_eq!(app.history.undo_label(), Some("Before"), "the drag left more than one undo step");

    app.run(Command::Redo);
    assert_eq!(app.scene.node(id).position, moved, "redo did not put the drag back in one");
}

/// Escape mid-drag restores the start position and leaves no undo step, which would restore
/// nothing visible.
#[test]
pub(crate) fn a_cancelled_drag_leaves_no_undo_step_behind() {
    let mut app = headless_app();
    let id = app.primary().unwrap();

    app.edit("Before", None);
    let steps_before = app.history.undo_label().map(str::to_string);
    let start = app.scene.node(id).position;

    let handle = Handle::MoveAxis(0);
    let gizmo = app.gizmo_for(id).unwrap();
    let view = app.current_view();
    let from = view.project(gizmo.handle_point(handle, &view)).unwrap().0;
    let to = view.project(gizmo.origin + Vec3::new(30.0, 0.0, 0.0)).unwrap().0;

    app.hover_handle = Some(handle);
    app.manipulate_step(&gizmo, &view, id, gizmo::DragPhase::Begin, Some(handle), Some(from), Default::default());
    app.manipulate_step(&gizmo, &view, id, gizmo::DragPhase::Continue, Some(handle), Some(to), Default::default());
    assert_ne!(app.scene.node(id).position, start, "the drag never got going, so cancelling proves nothing");

    app.manipulate_step(&gizmo, &view, id, gizmo::DragPhase::Cancel, Some(handle), Some(to), Default::default());
    assert_eq!(app.scene.node(id).position, start, "Escape did not restore the pre-drag position exactly");
    assert!(app.drag.is_none());
    assert_eq!(app.history.undo_label().map(str::to_string), steps_before, "the cancelled drag left an undo step");

    // The next undo reaches the edit before the drag.
    app.run(Command::Undo);
    assert!(app.history.undo_label().is_none());
}

/// The phase ordering that keeps one gesture to one undo step.
#[test]
pub(crate) fn a_second_undo_step_cannot_open_mid_gesture() {
    use gizmo::{drag_phase, DragPhase, PointerState};

    let grab = PointerState { started: true, on_handle: true, have_cursor: true, ..Default::default() };
    assert_eq!(drag_phase(false, grab), DragPhase::Begin);
    // A running drag must never report Begin again, which would split the gesture.
    assert_eq!(drag_phase(true, grab), DragPhase::Continue);

    // Escape beats release, so abandoning is never read as completing.
    let both = PointerState { escape: true, released: true, have_cursor: true, ..Default::default() };
    assert_eq!(drag_phase(true, both), DragPhase::Cancel);

    // A press off every handle starts nothing.
    assert_eq!(
        drag_phase(false, PointerState { started: true, have_cursor: true, ..Default::default() }),
        DragPhase::Idle
    );
    // The pointer leaving the window pauses the drag rather than ending it.
    assert_eq!(drag_phase(true, PointerState::default()), DragPhase::Idle);
    assert_eq!(drag_phase(false, PointerState::default()), DragPhase::Idle);
}
