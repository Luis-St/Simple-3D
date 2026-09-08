//! Reparenting, grouping and reordering.

use super::*;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn reparent_refuses_cycles_and_protects_the_root() {
    let mut scene = Scene::new();
    let root = scene.root();
    let outer = scene.add_group(GroupOp::Union, root, 0);
    let inner = scene.add_group(GroupOp::Union, outer, 0);

    assert!(scene.reparent(outer, inner, 0).is_err());
    assert!(scene.reparent(outer, outer, 0).is_err());
    assert!(scene.reparent(root, outer, 0).is_err());
    assert!(scene.reparent(inner, root, 0).is_ok());
    assert_eq!(scene.node(inner).parent, Some(root));
}

#[test]
pub(crate) fn reparent_within_one_parent_lands_where_the_indicator_showed() {
    let mut scene = Scene::new();
    let root = scene.root();
    let a = box_at(&mut scene, root, 0.0);
    let b = box_at(&mut scene, root, 1.0);
    let c = box_at(&mut scene, root, 2.0);
    // Drop `a` between `b` and `c`: index 2 in the pre-move list.
    scene.reparent(a, root, 2).unwrap();
    assert_eq!(scene.node(root).children, vec![b, a, c]);
}

#[test]
pub(crate) fn reparenting_several_nodes_keeps_their_order_and_lands_where_one_would() {
    // Issue 43: a multi-node drag is one move, not a loop of single moves --
    // each of those would shift the index the next was measured against.
    let mut scene = Scene::new();
    let root = scene.root();
    let a = box_at(&mut scene, root, 0.0);
    let b = box_at(&mut scene, root, 1.0);
    let c = box_at(&mut scene, root, 2.0);
    let d = box_at(&mut scene, root, 3.0);
    let group = scene.add_group(GroupOp::Union, root, 4);

    scene.reparent_many(&[a, c], group, 0).unwrap();
    assert_eq!(scene.node(group).children, vec![a, c], "the two lost their order on the way in");
    assert_eq!(scene.node(root).children, vec![b, d, group]);

    // Back out, between `b` and `d`: the index is read against the list as
    // it stands before the move, exactly as for a single node.
    scene.reparent_many(&[a, c], root, 1).unwrap();
    assert_eq!(scene.node(root).children, vec![b, a, c, d, group]);

    // A run that moves within one parent counts what leaves from in front
    // of the target, so it lands where the indicator was drawn.
    scene.reparent_many(&[b, a], root, 3).unwrap();
    assert_eq!(scene.node(root).children, vec![c, b, a, d, group]);
}

#[test]
pub(crate) fn a_multi_node_reparent_is_refused_whole_and_never_moves_a_carried_child() {
    let mut scene = Scene::new();
    let root = scene.root();
    let outer = scene.add_group(GroupOp::Union, root, 0);
    let inner = scene.add_group(GroupOp::Union, outer, 0);
    let leaf = box_at(&mut scene, inner, 0.0);
    let other = box_at(&mut scene, root, 1.0);

    // One illegal member fails the whole drag, and nothing has moved.
    assert!(scene.reparent_many(&[other, outer], inner, 0).is_err());
    assert_eq!(scene.node(root).children, vec![outer, other]);
    assert_eq!(scene.node(outer).children, vec![inner]);

    // A node inside another node being moved travels inside it; moving it
    // as well would tear it out of the group that carries it.
    scene.reparent_many(&[outer, leaf], root, 0).unwrap();
    assert_eq!(scene.node(inner).children, vec![leaf]);
    assert_eq!(scene.node(root).children, vec![outer, other]);
}

#[test]
pub(crate) fn grouping_a_selection_preserves_relative_positions_and_order() {
    // Spec acceptance criterion 10.
    let mut scene = Scene::new();
    let root = scene.root();
    let ids: Vec<NodeId> = (0..5).map(|i| box_at(&mut scene, root, i as f64 * 10.0)).collect();
    let before: Vec<Vec3> = ids.iter().map(|&id| scene.node(id).position).collect();

    let group = scene.group_selection(&ids).unwrap();
    assert_eq!(scene.node(group).children, ids);
    assert_eq!(scene.node(group).position, Vec3::ZERO);
    for (&id, &pos) in ids.iter().zip(before.iter()) {
        assert_eq!(scene.node(id).position, pos);
        assert_eq!(scene.node(id).parent, Some(group));
    }
    assert_eq!(scene.node(root).children, vec![group]);
}

#[test]
pub(crate) fn grouping_ignores_children_of_an_already_selected_group() {
    let mut scene = Scene::new();
    let root = scene.root();
    let outer = scene.add_group(GroupOp::Union, root, 0);
    let child = box_at(&mut scene, outer, 0.0);
    let sibling = box_at(&mut scene, root, 5.0);

    let group = scene.group_selection(&[outer, child, sibling]).unwrap();
    assert_eq!(scene.node(group).children, vec![outer, sibling]);
    assert_eq!(scene.node(child).parent, Some(outer));
}

#[test]
pub(crate) fn reorder_moves_within_the_parent_only() {
    let mut scene = Scene::new();
    let root = scene.root();
    let a = box_at(&mut scene, root, 0.0);
    let b = box_at(&mut scene, root, 1.0);
    assert!(scene.reorder(b, -1));
    assert_eq!(scene.node(root).children, vec![b, a]);
    assert!(!scene.reorder(b, -1));
    assert!(scene.reorder(b, 1));
    assert_eq!(scene.node(root).children, vec![a, b]);
}
