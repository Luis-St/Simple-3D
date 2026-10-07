//! The resize and scale popup (issue 120) and putting objects onto a face (issue 118).

use super::*;
use crate::arrange_tool::{Arrange, Landing};
use egui_kittest::kittest::Queryable;
use simple3d_core::config::SizeAnchor;
use simple3d_core::keymap::Command;
use simple3d_geom::Vec3;

/// A harness with a box at the origin, selected and evaluated.
fn with_box(name: &str) -> Harness<'static, App> {
    harness_configured(name, |app| {
        let root = app.scene.root();
        let id = app.scene.add_primitive("box", root, 1).expect("the box is in the registry");
        app.selection = vec![id];
        app.evaluated = Evaluator::new().evaluate(&app.scene, &Cancel::new());
    })
}

#[test]
pub(crate) fn the_resize_tool_shows_its_popup_until_closed_and_again_when_picked() {
    let mut harness = with_box("size-popup");
    harness.state_mut().run(Command::ModeResize);
    for _ in 0..4 {
        harness.step();
    }
    assert!(harness.query_by_label("Stays put").is_some(), "the resize popup did not open");
    harness.get_by_label("Close").click();
    harness.step();
    harness.step();
    assert!(harness.query_by_label("Stays put").is_none(), "Close left the popup open");
    harness.state_mut().run(Command::ModeScale);
    harness.step();
    harness.step();
    assert!(harness.query_by_label("Stays put").is_some(), "picking scale did not bring it back");
}

#[test]
pub(crate) fn a_typed_size_grows_about_the_centre() {
    let mut harness = with_box("size-typed");
    harness.state_mut().run(Command::ModeResize);
    harness.step();
    let app = harness.state_mut();
    let id = app.primary().unwrap();
    let (lo, hi) = app.evaluated.node_world_bounds[&id];
    app.set_size(0, 50.0);
    // Scrubbed on, measured from where the run began rather than from the stale evaluation.
    app.set_size(0, 60.0);
    app.evaluated = Evaluator::new().evaluate(&app.scene, &Cancel::new());
    let (lo_after, hi_after) = app.evaluated.node_world_bounds[&id];
    let moved = (lo_after.x + hi_after.x - lo.x - hi.x) / 2.0;
    assert!(moved.abs() < 1e-9, "the centre moved by {moved}");
    assert!((hi_after.x - lo_after.x - 60.0).abs() < 1e-9, "the width is {}", hi_after.x - lo_after.x);
    assert_eq!(app.history.undo_len(), 1, "the scrub is not one undo step");
}

#[test]
pub(crate) fn the_anchor_chips_set_every_axis_at_once() {
    let mut harness = with_box("size-chips");
    harness.state_mut().run(Command::ModeScale);
    for _ in 0..4 {
        harness.step();
    }
    harness.state_mut().settings.size_anchor = SizeAnchor::Centre;
    harness.step();
    harness.get_by_label("Opposite side").click();
    harness.step();
    assert_eq!(harness.state().settings.size_anchor, SizeAnchor::Opposite);
    let id = harness.state().primary().unwrap();
    assert_eq!(harness.state().gizmo_for(id).unwrap().anchor, SizeAnchor::Opposite, "the handles do not follow it");
}

#[test]
pub(crate) fn putting_a_box_onto_a_face_through_the_window_rests_it_there() {
    let mut harness = with_box("onto-face");
    harness.state_mut().run(Command::OntoFace);
    for _ in 0..4 {
        harness.step();
    }
    let tool = harness.state_mut().arrange_tool.as_mut().expect("the tool did not open");
    assert_eq!(tool.mode, Arrange::OntoFace);
    let top = Vec3::new(100.0, 0.0, 30.0);
    tool.landing = Some(Landing { centre: top, at: top, normal: Vec3::new(0.0, 0.0, 1.0) });
    harness.step();
    harness.get_by_label("Apply").click();
    harness.step();
    harness.step();
    let app = harness.state();
    let id = app.primary().unwrap();
    let position = app.scene.node(id).position;
    assert!((position.x - 100.0).abs() < 1e-9 && position.y.abs() < 1e-9, "not over the face: {position:?}");
    let half = app.evaluated.node_local_bounds[&id].1.z;
    assert!((position.z - 30.0 - half).abs() < 1e-9, "not resting on the face: {position:?}");
}
