//! Holding the snap key through a drag.

use super::*;
use simple3d_core::eval::Evaluator;
use simple3d_geom::Vec3;

/// Issue 76 and issue 68 together, through the window: holding the snap key --
/// Ctrl on its own, which is only bindable at all because of issue 76 -- during
/// a move drag is what makes the drag snap to another body's geometry.
///
/// Every other test of this reaches one half of it: `geometry_snap_wanted` is
/// asked directly whether Ctrl means snap, and the drag arithmetic is driven
/// with `snap_requested` already set. Nothing until this drove a hand holding
/// Ctrl and dragging, which is the way anybody actually meets the feature.
#[test]
pub(crate) fn holding_the_snap_key_through_a_drag_snaps_to_another_body() {
    let mut harness = harness_configured("snap-while-held", |app| {
        app.settings.geometry_snap = simple3d_core::config::SnapMode::WhileHeld;
        // Two boxes rather than the starting plate: a small body to carry, and
        // one to snap it to, well clear of it so the target is unambiguous. The
        // grid step is coarse so a grid landing and a geometry landing cannot be
        // the same number by accident.
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
    // Dragged across the other box a step at a time: somewhere along the way the
    // pointer passes one of its corners, and that is what the carried box lands
    // on.
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
    // The grid step is 10 mm, so a landing off it is one only the geometry can
    // have chosen.
    assert!(
        (landed.x / 10.0).fract().abs() > 1e-6,
        "the drag landed on the grid step at {landed:?}, so nothing snapped to the body"
    );
}

/// Issue 68, the whole point of it: two bodies brought face to face.
///
/// Placing a part against another is what geometry snapping is *for*, and it is
/// the case the feature could not do. The target used to be whatever feature the
/// *pointer* was over, and the manipulator handle is grabbed some seventy pixels
/// out from the body -- so by the time the pointer reached the corner to meet,
/// the body it was carrying had already been dragged on top of that corner. Two
/// 20 mm boxes could be snapped into the same 20 mm of space, and into nothing
/// else: the landing where their faces touch was never once offered.
///
/// The carried box starts at the origin and the target sits at 75, so the two
/// stand 55 mm apart with a 20 mm box between them. Their faces meet when the
/// carried box is at 55, which is the number this drag has to be able to reach.
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
        // A grid step that cannot land on 55 by itself, so a landing there is one
        // the geometry chose.
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
    // Crossed the whole gap a millimetre at a time, keeping every place the drag
    // snapped to. A person doing this by eye stops at the one they wanted; the
    // test only has to prove it was offered at all.
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
