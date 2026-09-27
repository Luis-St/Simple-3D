//! Holding the snap key through a drag.

use super::*;
use simple3d_core::eval::Evaluator;
use simple3d_geom::Vec3;

/// Issues 76 and 68 through the window: holding Ctrl alone during a move drag snaps to another
/// body's geometry. Other tests cover each half; this drives the actual gesture.
#[test]
pub(crate) fn holding_the_snap_key_through_a_drag_snaps_to_another_body() {
    let mut harness = harness_configured("snap-while-held", |app| {
        app.settings.geometry_snap = simple3d_core::config::SnapMode::WhileHeld;
        // Two boxes, well apart so the target is unambiguous; a coarse grid step so grid and geometry
        // landings cannot coincide.
        let root = app.scene.root();
        for id in app.scene.node(root).children.clone() {
            app.scene.remove(id);
        }
        let carried = app.scene.add_primitive("box", root, 0).expect("the box is in the registry");
        let target = app.scene.add_primitive("box", root, 1).expect("the box is in the registry");
        app.scene.get_mut(target).unwrap().position = Vec3::new(75.0, 0.0, 0.0);
        app.scene.settings.snap_step = 10.0;
        app.select_only(carried);
    });
    let carried = harness.state().primary().unwrap();
    harness.state_mut().mode = crate::gizmo::Mode::Move;
    harness.step();
    harness.state_mut().evaluated =
        Evaluator::new().evaluate(&harness.state().scene, &simple3d_core::eval::Cancel::new());
    harness.state_mut().frame_all();
    harness.step();

    let view = harness.state().current_view();
    let gizmo = harness.state().gizmo_for(carried).expect("a gizmo for the selected box");
    let handle = crate::gizmo::Handle::MoveAxis(0);
    let (at, _) = view.project(gizmo.handle_point(handle, &view)).expect("the X arrow is off screen");

    modifiers(&mut harness, egui::Modifiers::COMMAND);
    press(&mut harness, at);
    move_to(&mut harness, at + egui::vec2(12.0, 0.0));
    // Dragged across the other box step by step, so the pointer passes one of its corners.
    let mut snapped = false;
    let mut landed = Vec3::ZERO;
    for step in 1..=16 {
        let to = view.project(gizmo.handle_point(handle, &view) + gizmo.axes[0] * (step as f64 * 6.0)).unwrap().0;
        move_to(&mut harness, to);
        if harness.state().snap_indicator.is_some() {
            snapped = true;
            landed = harness.state().scene.node(carried).position;
        }
    }
    assert!(harness.state().snap_requested, "Ctrl held through the drag did not ask for a geometry snap");
    release(&mut harness, at + egui::vec2(300.0, 0.0));
    modifiers(&mut harness, egui::Modifiers::NONE);
    harness.step();

    assert!(snapped, "the drag never met a feature of the other body to snap to");
    // The grid step is 10 mm, so an off-grid landing must be the geometry's.
    assert!(
        (landed.x / 10.0).fract().abs() > 1e-6,
        "the drag landed on the grid step at {landed:?}, so nothing snapped to the body"
    );
}

/// Issue 68: two bodies brought face to face. Pointer-based targets could never offer the touching
/// landing, since the handle is grabbed far out from the body. With the carried box at the origin
/// and the target at 75, the faces meet at 55, which this drag must reach.
#[test]
pub(crate) fn a_snapped_drag_can_put_two_boxes_face_to_face() {
    let mut harness = harness_configured("snap-face-to-face", |app| {
        app.settings.geometry_snap = simple3d_core::config::SnapMode::WhileHeld;
        let root = app.scene.root();
        for id in app.scene.node(root).children.clone() {
            app.scene.remove(id);
        }
        let carried = app.scene.add_primitive("box", root, 0).expect("the box is in the registry");
        let target = app.scene.add_primitive("box", root, 1).expect("the box is in the registry");
        app.scene.get_mut(target).unwrap().position = Vec3::new(75.0, 0.0, 0.0);
        // A grid step that cannot land on 55 by itself.
        app.scene.settings.snap_step = 10.0;
        app.select_only(carried);
    });
    let carried = harness.state().primary().unwrap();
    harness.state_mut().mode = crate::gizmo::Mode::Move;
    harness.step();
    harness.state_mut().evaluated =
        Evaluator::new().evaluate(&harness.state().scene, &simple3d_core::eval::Cancel::new());
    harness.state_mut().frame_all();
    harness.step();

    let view = harness.state().current_view();
    let gizmo = harness.state().gizmo_for(carried).expect("a gizmo for the selected box");
    let handle = crate::gizmo::Handle::MoveAxis(0);
    let start = gizmo.handle_point(handle, &view);
    let (at, _) = view.project(start).expect("the X arrow is off screen");

    modifiers(&mut harness, egui::Modifiers::COMMAND);
    press(&mut harness, at);
    // Cross the gap a millimetre at a time, collecting every landing offered.
    let mut landings: Vec<f64> = Vec::new();
    for step in 1..=90 {
        let to = view.project(start + gizmo.axes[0] * step as f64).expect("the drag ran off screen").0;
        move_to(&mut harness, to);
        if harness.state().snap_indicator.is_some() {
            landings.push(harness.state().scene.node(carried).position.x);
        }
    }
    release(&mut harness, at);
    modifiers(&mut harness, egui::Modifiers::NONE);
    harness.step();

    assert!(
        landings.iter().any(|x| (x - 55.0).abs() < 1e-6),
        "the drag never offered the landing where the two boxes touch; it snapped to {landings:?}"
    );
}

/// Ctrl is both the default snap key (issue 77) and the symmetric-resize modifier; where they
/// collide the snap wins and the far face stays put.
#[test]
pub(crate) fn a_face_pulled_with_the_snap_key_held_does_not_resize_about_the_centre() {
    let mut harness = harness_configured("snap-resize-ctrl", |app| {
        app.settings.geometry_snap = simple3d_core::config::SnapMode::WhileHeld;
        let root = app.scene.root();
        for id in app.scene.node(root).children.clone() {
            app.scene.remove(id);
        }
        // Alone in the scene, so the drag is a plain grid resize and only Ctrl's effect is tested.
        let block = app.scene.add_primitive("box", root, 0).expect("the box is in the registry");
        app.select_only(block);
    });
    let block = harness.state().primary().unwrap();
    harness.state_mut().mode = crate::gizmo::Mode::Resize;
    harness.step();
    harness.state_mut().evaluated =
        Evaluator::new().evaluate(&harness.state().scene, &simple3d_core::eval::Cancel::new());
    harness.state_mut().frame_all();
    harness.step();

    let view = harness.state().current_view();
    let gizmo = harness.state().gizmo_for(block).expect("a gizmo for the selected box");
    let handle = crate::gizmo::Handle::ResizeFace(0, true);
    let start = gizmo.handle_point(handle, &view);
    let (at, _) = view.project(start).expect("the +X face handle is off screen");

    modifiers(&mut harness, egui::Modifiers::COMMAND);
    press(&mut harness, at);
    for step in 1..=10 {
        let to = view.project(start + gizmo.axes[0] * step as f64).expect("the drag ran off screen").0;
        move_to(&mut harness, to);
    }
    assert!(harness.state().snap_requested, "Ctrl held through the drag did not ask for a geometry snap");
    release(&mut harness, view.project(start + gizmo.axes[0] * 10.0).unwrap().0);
    modifiers(&mut harness, egui::Modifiers::NONE);
    harness.step();

    let evaluated = Evaluator::new().evaluate(&harness.state().scene, &simple3d_core::eval::Cancel::new());
    let (lo, hi) = evaluated.node_world_bounds[&block];
    assert!(hi.x > 12.0, "the drag did not pull the +X face out: {lo:?}..{hi:?}");
    assert!((lo.x + 10.0).abs() < 1e-6, "the -X face moved to {} as well: Ctrl resized about the centre", lo.x);
}
