//! Adding, duplicating and deleting.

use super::*;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn add_targets_the_selected_group_or_follows_a_leaf() {
    let mut scene = Scene::new();
    let root = scene.root();
    let a = box_at(&mut scene, root, 0.0);
    let group = scene.add_group(GroupOp::Union, root, 1);

    assert_eq!(scene.insertion_point(Some(group)), (group, 0));
    assert_eq!(scene.insertion_point(Some(a)), (root, 1));
    assert_eq!(scene.insertion_point(None), (root, 2));
}

#[test]
pub(crate) fn duplicate_is_a_deep_copy_with_distinct_identity() {
    // Spec acceptance criterion 20/21.
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Difference, root, 0);
    let child = box_at(&mut scene, group, 5.0);
    scene.get_mut(child).unwrap().anchor = Anchor::Base;
    scene.get_mut(child).unwrap().segments = Some(64);
    scene.get_mut(child).unwrap().visible = false;

    let copy = scene.duplicate(group).unwrap();
    assert_ne!(copy, group);
    assert_eq!(scene.node(root).children, vec![group, copy]);
    assert_eq!(scene.node(copy).group_op(), Some(GroupOp::Difference));
    let copied_child = scene.node(copy).children[0];
    assert_ne!(copied_child, child);
    assert_eq!(scene.node(copied_child).position, Vec3::new(5.0, 0.0, 0.0));
    assert_eq!(scene.node(copied_child).anchor, Anchor::Base);
    assert_eq!(scene.node(copied_child).segments, Some(64));
    assert!(!scene.node(copied_child).visible);

    // Editing the copy leaves the original untouched.
    scene.get_mut(copied_child).unwrap().position = Vec3::new(99.0, 0.0, 0.0);
    assert_eq!(scene.node(child).position, Vec3::new(5.0, 0.0, 0.0));
}

#[test]
pub(crate) fn deleting_the_root_is_refused_and_subtrees_go_entirely() {
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Union, root, 0);
    let child = box_at(&mut scene, group, 0.0);
    assert!(!scene.remove(root));
    assert!(scene.remove(group));
    assert!(!scene.contains(child));
    assert_eq!(scene.len(), 1);
}

#[test]
pub(crate) fn difference_base_is_the_first_visible_child() {
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Difference, root, 0);
    let a = box_at(&mut scene, group, 0.0);
    let b = box_at(&mut scene, group, 1.0);
    assert_eq!(scene.difference_base(group), Some(a));
    scene.get_mut(a).unwrap().visible = false;
    assert_eq!(scene.difference_base(group), Some(b));
}

#[test]
pub(crate) fn ids_are_never_reused_after_deletion() {
    let mut scene = Scene::new();
    let root = scene.root();
    let a = box_at(&mut scene, root, 0.0);
    scene.remove(a);
    let b = box_at(&mut scene, root, 0.0);
    assert_ne!(a, b);
}
