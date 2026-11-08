//! The brush's pointer (issue 127): strokes, Escape, and the wheel.

use super::*;
use simple3d_core::keymap::Command;
use simple3d_core::mesh_data::MeshData;
use simple3d_core::scene::NodeId;
use simple3d_geom::Vec3;

/// The harness's shape turned into a converted sphere, 60 mm across, with the brush picked.
fn brushing(name: &str) -> (Harness<'static, App>, NodeId) {
    let mut harness = harness_configured(name, |app| {
        let id = app.selection[0];
        let sphere = MeshData::new(simple3d_geom::primitives::ellipsoid_mesh(60.0, 60.0, 60.0, 96));
        assert!(app.scene.convert_to_mesh(id, sphere));
        app.evaluated = Evaluator::new().evaluate(&app.scene, &Cancel::new());
        app.frame_all();
    });
    harness.state_mut().run(Command::ModeBrush);
    harness.step();
    let id = harness.state().selection[0];
    (harness, id)
}

fn positions(harness: &Harness<'_, App>, id: NodeId) -> Vec<Vec3> {
    harness.state().scene.node(id).mesh().expect("a mesh").mesh.positions.clone()
}

/// The sphere's centre on screen, and a point a little to its side.
fn across(harness: &Harness<'_, App>) -> (egui::Pos2, egui::Pos2) {
    let app = harness.state();
    let (lo, hi) = app.evaluated.bounds.expect("evaluated");
    let centre = app.current_view().project((lo + hi) * 0.5).expect("in view").0;
    (centre - egui::vec2(40.0, 0.0), centre + egui::vec2(40.0, 0.0))
}

#[test]
fn a_drag_over_the_mesh_shapes_it_in_one_undo_step_and_only_when_released() {
    let (mut harness, id) = brushing("brush-stroke");
    let before = positions(&harness, id);
    let (from, to) = across(&harness);
    press(&mut harness, from);
    for k in 1..=8 {
        move_to(&mut harness, from + (to - from) * (k as f32 / 8.0));
    }
    let app = harness.state();
    assert!(app.brush.stroke.is_some(), "the press on the mesh did not start a stroke");
    assert!(app.sculpt_shown().is_some(), "the working copy is not drawn during the stroke");
    assert_eq!(positions(&harness, id), before, "the document changed before the stroke ended");
    assert_eq!(harness.state().history.undo_len(), 0);
    release(&mut harness, to);
    let after = positions(&harness, id);
    assert_ne!(after, before, "the stroke did not reach the document");
    assert_eq!(harness.state().history.undo_len(), 1, "a stroke took other than one undo step");
    // Raised: the moved vertices went outwards, and the sphere stays closed.
    let grew = before.iter().zip(&after).filter(|(a, b)| b.length() > a.length() + 1e-9).count();
    assert!(grew > 10, "only {grew} vertices rose");
    let mesh = &harness.state().scene.node(id).mesh().expect("a mesh").mesh;
    assert!(mesh.manifold_issue().is_none());
    // A click selects nothing else in the brush: the selection stays on the mesh.
    assert_eq!(harness.state().selection, vec![id]);
}

#[test]
fn a_press_on_an_unselected_mesh_selects_and_brushes_it() {
    let (mut harness, id) = brushing("brush-unselected");
    harness.state_mut().selection.clear();
    harness.step();
    let before = positions(&harness, id);
    let (from, to) = across(&harness);
    press(&mut harness, from);
    for k in 1..=8 {
        move_to(&mut harness, from + (to - from) * (k as f32 / 8.0));
    }
    release(&mut harness, to);
    assert_eq!(harness.state().selection, vec![id], "the press did not select the mesh");
    assert_ne!(positions(&harness, id), before, "the stroke did not shape the unselected mesh");
}

/// How far the sphere's furthest vertex is from its centre after pressing at `at` for `frames` frames
/// without moving.
fn held(name: &str, frames: usize) -> f64 {
    let (mut harness, id) = brushing(name);
    let (at, _) = across(&harness);
    press(&mut harness, at);
    for _ in 0..frames {
        harness.step();
    }
    release(&mut harness, at);
    assert_eq!(harness.state().history.undo_len(), 1, "not one undo step");
    positions(&harness, id).iter().map(|p| p.length()).fold(0.0, f64::max)
}

#[test]
fn a_press_held_still_keeps_raising_where_it_is() {
    // Holding still laid no dab at all: dabs came only when the pointer had moved a spacing on.
    let click = held("brush-click", 0);
    let hold = held("brush-hold", 20);
    assert!(click > 30.0 + 1e-6, "a click did not raise the sphere: {click}");
    assert!(hold > click + 0.5, "holding raised no more than a click: {hold} against {click}");
}

#[test]
fn escape_abandons_a_stroke_without_a_trace() {
    let (mut harness, id) = brushing("brush-escape");
    let before = positions(&harness, id);
    let (from, to) = across(&harness);
    press(&mut harness, from);
    for k in 1..=4 {
        move_to(&mut harness, from + (to - from) * (k as f32 / 4.0));
    }
    key(&mut harness, egui::Key::Escape);
    release(&mut harness, to);
    assert!(harness.state().brush.stroke.is_none());
    assert_eq!(positions(&harness, id), before);
    assert_eq!(harness.state().history.undo_len(), 0);
    let sculpt = harness.state().sculpt.as_ref().expect("the session stays for the next stroke");
    assert!(sculpt.stroke.is_none() && !sculpt.shown());
}

#[test]
fn ctrl_and_the_wheel_size_the_brush_instead_of_zooming() {
    let (mut harness, _) = brushing("brush-wheel");
    let (at, _) = across(&harness);
    move_to(&mut harness, at);
    let (distance, radius) = (harness.state().scene.camera.distance, harness.state().settings.last_brush.radius);
    modifiers(&mut harness, egui::Modifiers::COMMAND);
    let delta = egui::vec2(0.0, 120.0);
    event(
        &mut harness,
        egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta, modifiers: egui::Modifiers::COMMAND },
    );
    for _ in 0..6 {
        harness.step();
    }
    modifiers(&mut harness, egui::Modifiers::NONE);
    let app = harness.state();
    assert!(app.settings.last_brush.radius > radius, "the brush did not grow");
    assert_eq!(app.scene.camera.distance, distance, "the view zoomed too");
}

#[test]
fn alt_b_picks_the_brush() {
    let mut harness = harness("brush-key");
    modifiers(&mut harness, egui::Modifiers::ALT);
    key(&mut harness, egui::Key::B);
    modifiers(&mut harness, egui::Modifiers::NONE);
    assert_eq!(harness.state().mode, crate::gizmo::Mode::Brush);
}
