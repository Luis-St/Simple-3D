use super::drag::*;
use super::row_parts::*;
use crate::app::DropTarget;
use crate::icon::Glyph;
use simple3d_core::scene::{NodeId, Scene};

use simple3d_core::scene::GroupOp;

fn tree() -> (Scene, NodeId, NodeId, NodeId, NodeId) {
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Union, root, 0);
    let inner = scene.add_primitive("box", group, 0).unwrap();
    let sibling = scene.add_primitive("box", root, 1).unwrap();
    (scene, root, group, inner, sibling)
}

#[test]
fn the_middle_of_a_group_row_drops_into_it() {
    let (scene, root, group, _, _) = tree();
    let target = drop_position(&scene, group, 0.5, root, true).unwrap();
    assert_eq!(target.into, Some(group));
    assert_eq!(target.parent, group);
}

#[test]
fn the_edges_of_a_row_drop_beside_it() {
    let (scene, root, group, _, sibling) = tree();
    let before = drop_position(&scene, sibling, 0.05, root, false).unwrap();
    assert_eq!(before.parent, root);
    assert_eq!(before.index, 1);
    assert_eq!(before.into, None);

    let after = drop_position(&scene, sibling, 0.95, root, false).unwrap();
    assert_eq!(after.index, 2);

    // A group's edges also mean "beside", not "into".
    let beside_group = drop_position(&scene, group, 0.05, root, true).unwrap();
    assert_eq!(beside_group.parent, root);
    assert_eq!(beside_group.index, 0);
    assert_eq!(beside_group.into, None);
}

#[test]
fn the_gap_under_an_open_group_lands_inside_it() {
    // Issue 49: dragged into the gap between a group's row and its first
    // child, an item was landing beside the group in the group's parent --
    // a level out from where the line was drawn. It now becomes the
    // group's first child.
    let (scene, root, group, inner, _) = tree();
    let target = drop_position(&scene, group, 0.95, root, true).unwrap();
    assert_eq!(target.parent, group);
    assert_eq!(target.index, 0);
    assert_eq!(target.into, None);
    // And that index is the one the move honours.
    let mut scene = scene;
    let sibling = scene.node(root).children[1];
    scene.reparent(sibling, target.parent, target.index).unwrap();
    assert_eq!(scene.node(group).children, vec![sibling, inner]);
}

#[test]
fn the_gap_under_a_shut_group_still_lands_beside_it() {
    // Nothing of the group is shown under its row while it is collapsed,
    // so the gap there is the one between it and its next sibling.
    let (scene, root, group, _, _) = tree();
    let target = drop_position(&scene, group, 0.95, root, false).unwrap();
    assert_eq!(target.parent, root);
    assert_eq!(target.index, 1);
    assert_eq!(target.into, None);
}

#[test]
fn one_gap_between_two_rows_draws_one_line() {
    // Issue 48: the same landing place -- under the upper row, over the
    // lower one -- was drawn at two heights a spacing apart, so slowly
    // crossing a gap showed two orange lines and read as two positions to
    // aim at. Both now land in the middle of the gap, which is one line.
    let spacing = 4.0;
    let upper = egui::Rect::from_min_size(egui::pos2(0.0, 10.0), egui::vec2(100.0, 22.0));
    let lower = egui::Rect::from_min_size(egui::pos2(0.0, upper.bottom() + spacing), egui::vec2(100.0, 22.0));
    assert_eq!(gap_line_y(upper, spacing, true), gap_line_y(lower, spacing, false));
    // And it is the gap it names, not the inside of either row.
    let y = gap_line_y(upper, spacing, true);
    assert!(y > upper.bottom() && y < lower.top(), "the line at {y} is not in the gap");
}

#[test]
fn a_leaf_row_never_drops_into_itself() {
    let (scene, root, _, inner, _) = tree();
    for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let target = drop_position(&scene, inner, fraction, root, false).unwrap();
        assert_eq!(target.into, None, "a leaf accepted a child at {fraction}");
    }
}

#[test]
fn any_drop_on_the_root_goes_inside_it() {
    let (scene, root, _, _, _) = tree();
    for fraction in [0.0, 0.5, 1.0] {
        let target = drop_position(&scene, root, fraction, root, false).unwrap();
        assert_eq!(target.parent, root);
        assert_eq!(target.into, Some(root));
    }
}

#[test]
fn the_index_a_drop_reports_is_the_one_reparent_expects() {
    // The two have to agree, or a drop lands one row away from the indicator.
    let (mut scene, root, group, inner, sibling) = tree();
    let target = drop_position(&scene, sibling, 0.95, root, false).unwrap();
    scene.reparent(inner, target.parent, target.index).unwrap();
    assert_eq!(scene.node(root).children, vec![group, sibling, inner]);
}

#[test]
fn a_drop_is_refused_when_any_one_of_the_dragged_nodes_would_swallow_itself() {
    // Issue 43: the whole load is judged, not just the row that was grabbed.
    let (scene, root, group, inner, sibling) = tree();
    let into_group = DropTarget { parent: group, index: 0, into: Some(group) };
    assert!(drop_is_legal(&scene, &[sibling], &into_group));
    // `group` is being dragged too, so nothing may land inside it.
    assert!(!drop_is_legal(&scene, &[sibling, group], &into_group));
    // Nor may a node land inside itself, however it was reached.
    assert!(!drop_is_legal(&scene, &[group], &into_group));
    // Its own child is a fine place for a sibling, and not for the group.
    let beside_inner = DropTarget { parent: group, index: 1, into: None };
    assert!(drop_is_legal(&scene, &[sibling], &beside_inner));
    assert!(!drop_is_legal(&scene, &[inner, group], &beside_inner));
    // Only a group can hold children.
    let into_leaf = DropTarget { parent: sibling, index: 0, into: Some(sibling) };
    assert!(!drop_is_legal(&scene, &[inner], &into_leaf));
    let _ = root;
}

#[test]
fn a_group_wears_its_own_operator_and_a_cut_child_wears_the_cut() {
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Difference, root, 0);
    let base = scene.add_primitive("box", group, 0).unwrap();
    let cut = scene.add_primitive("cylinder", group, 1).unwrap();

    assert_eq!(operator_badge(&scene, group), Some((Glyph::Difference, false)));
    // The base is what is being cut, so it carries no mark ...
    assert_eq!(operator_badge(&scene, base), None);
    // ... and the operand that does the cutting is marked, in the danger
    // colour, which is what `true` here selects.
    assert_eq!(operator_badge(&scene, cut), Some((Glyph::Difference, true)));
}

#[test]
fn the_root_row_carries_no_operator_mark() {
    let (scene, root, _, _, _) = tree();
    assert_eq!(operator_badge(&scene, root), None);
}

#[test]
fn a_child_of_a_union_carries_no_mark_of_its_own() {
    // Union is the default and the common case; badging every child of one
    // would be noise down the whole tree.
    let (scene, _, _, inner, _) = tree();
    assert_eq!(operator_badge(&scene, inner), None);
}
