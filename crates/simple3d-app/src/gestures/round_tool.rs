//! The edge rounding tool's picks, sizing drag, draft and apply (issue 88).

use super::*;
use crate::round_tool::Kind;
use simple3d_core::keymap::Command;
use simple3d_core::scene::ObjectEdit;
use simple3d_geom::Vec3;

/// A top edge of the plate the camera sees, on screen.
fn a_top_edge(harness: &Harness<'_, App>) -> egui::Pos2 {
    let app = harness.state();
    let view = app.current_view();
    let (lo, hi) = app.evaluated.bounds.unwrap();
    let mid = (lo + hi) * 0.5;
    [
        Vec3::new(mid.x, lo.y, hi.z),
        Vec3::new(mid.x, hi.y, hi.z),
        Vec3::new(lo.x, mid.y, hi.z),
        Vec3::new(hi.x, mid.y, hi.z),
    ]
    .into_iter()
    .filter_map(|p| view.project(p).map(|(s, _)| s))
    .find(|&s| matches!(app.round_pick_at(&view, s), Some(crate::round_tool::pick::Pick::Edge(_))))
    .expect("no top edge in sight")
}

pub(super) fn click(harness: &mut Harness<'_, App>, at: egui::Pos2) {
    press(harness, at);
    release(harness, at);
}

fn plate(harness: &Harness<'_, App>) -> simple3d_core::scene::NodeId {
    harness.state().scene.node(harness.state().scene.root()).children[0]
}

#[test]
pub(crate) fn a_picked_edge_is_bevelled_by_an_edit_kept_on_the_plate() {
    let mut harness = harness("round-tool");
    harness.state_mut().run(Command::RoundEdges);
    harness.step();
    let plate = plate(&harness);
    let before = harness.state().evaluated.mesh.signed_volume();
    let at = a_top_edge(&harness);
    click(&mut harness, at);
    assert_eq!(harness.state().round_tool.as_ref().unwrap().edges.len(), 1, "the click did not pick the edge");
    // The pick stands in the document as a draft, which is no undo step, with its ghost drawn red.
    assert_eq!(harness.state().scene.node(plate).edits.len(), 1, "the pick made no draft");
    assert_eq!(harness.state().history.undo_len(), 0);
    let palette = crate::render::Palette::dark();
    let colours: Vec<_> =
        crate::round_tool::templates(harness.state().round_tool.as_ref(), &palette).map(|t| t.2).collect();
    assert_eq!(colours, vec![Some(palette.reduction)]);
    // Clicking it again drops it, draft and all, and once more picks it back.
    click(&mut harness, at);
    assert!(harness.state().round_tool.as_ref().unwrap().edges.is_empty(), "a second click kept the edge");
    assert!(harness.state().scene.node(plate).edits.is_empty(), "the dropped pick left its draft");
    click(&mut harness, at);

    {
        let tool = harness.state_mut().round_tool.as_mut().unwrap();
        tool.kind = Kind::Chamfer;
        tool.distance = 1.0;
    }
    harness.state_mut().apply_round_tool();
    let app = harness.state();
    assert!(app.round_tool.is_none(), "Apply left the tool open");
    assert_eq!(app.scene.node(app.scene.root()).children, vec![plate], "the bevel made a node of its own");
    let edits = &app.scene.node(plate).edits;
    assert!(matches!(&edits[..], [ObjectEdit::Round(edit)] if edit.kind == Kind::Chamfer && edit.size == 1.0));
    assert_eq!(app.history.undo_len(), 1);
    assert_eq!(app.selection.last(), Some(&plate));

    let after = Evaluator::new().evaluate(&app.scene, &Cancel::new());
    assert!(after.errors.is_empty(), "{:?}", after.errors);
    assert!(after.mesh.manifold_issue().is_none());
    let (lo, hi) = app.evaluated.bounds.unwrap();
    let size = hi - lo;
    // One chamfer of 1 mm along a top edge takes half a square millimetre per millimetre of edge.
    let removed = before - after.mesh.signed_volume();
    let lengths = [size.x, size.y];
    assert!(lengths.iter().any(|l| (removed - 0.5 * l).abs() < 1e-3), "removed {removed} of {size:?}");

    // Undone, the plate is as it was.
    harness.state_mut().run(Command::Undo);
    assert!(harness.state().scene.node(plate).edits.is_empty());
}

#[test]
pub(crate) fn dragging_an_edge_picks_it_and_sizes_the_treatment_in_steps() {
    let mut harness = harness("round-tool-drag");
    harness.state_mut().run(Command::RoundEdges);
    harness.step();
    let at = a_top_edge(&harness);
    press(&mut harness, at);
    for k in 1..=6 {
        move_to(&mut harness, at + egui::vec2(8.0 * k as f32, 8.0 * k as f32));
    }
    let app = harness.state();
    let tool = app.round_tool.as_ref().unwrap();
    assert!(tool.drag.is_some(), "the press on the edge did not take hold of it");
    assert_eq!(tool.edges.len(), 1, "the dragged edge was not picked");
    let step = app.scene.settings.snap_step;
    let radius = tool.radius;
    assert!(radius >= step && ((radius / step).round() * step - radius).abs() < 1e-9, "radius {radius}");
    assert_ne!(radius, 2.0, "the drag did not size the rounding");

    // Escape puts the size back but keeps the pick; letting go then changes nothing more.
    key(&mut harness, egui::Key::Escape);
    release(&mut harness, at + egui::vec2(48.0, 48.0));
    let tool = harness.state().round_tool.as_ref().unwrap();
    assert!(tool.drag.is_none());
    assert_eq!(tool.radius, 2.0);
    assert_eq!(tool.edges.len(), 1);
}

#[test]
pub(crate) fn escape_closes_the_tool_and_takes_the_draft_away() {
    let mut harness = harness("round-tool-escape");
    harness.state_mut().run(Command::RoundEdges);
    harness.step();
    let at = a_top_edge(&harness);
    click(&mut harness, at);
    assert!(!harness.state().scene.node(plate(&harness)).edits.is_empty());
    key(&mut harness, egui::Key::Escape);
    assert!(harness.state().round_tool.is_none());
    assert!(harness.state().scene.node(plate(&harness)).edits.is_empty(), "the draft stayed in the document");
    assert_eq!(harness.state().history.undo_len(), 0);
}

#[test]
pub(crate) fn an_edit_made_beside_the_tool_snapshots_the_document_without_the_draft() {
    let mut harness = harness("round-tool-beside");
    harness.state_mut().run(Command::RoundEdges);
    harness.step();
    let at = a_top_edge(&harness);
    click(&mut harness, at);
    let plate = plate(&harness);
    harness.state_mut().edit("Move", None);
    harness.state_mut().scene.get_mut(plate).unwrap().position.x += 5.0;
    assert_eq!(harness.state().scene.node(plate).edits.len(), 1, "the draft did not come back");
    harness.state_mut().run(Command::Undo);
    let app = harness.state();
    assert!(app.round_tool.is_none(), "undo left the tool open over a document it no longer matches");
    assert!(app.scene.node(plate).edits.is_empty(), "the undo step held the draft");
}
