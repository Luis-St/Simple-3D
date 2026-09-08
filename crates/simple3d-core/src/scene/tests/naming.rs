//! The names duplicates and imports end up with.

use super::*;

#[test]
pub(crate) fn a_duplicate_is_named_as_a_copy() {
    // A duplicate is the same thing the clipboard makes, so it reads the
    // same way; two siblings both called "Box" say nothing about which is
    // which.
    let mut scene = Scene::new();
    let root = scene.root();
    let a = box_at(&mut scene, root, 0.0);
    let copy = scene.duplicate(a).unwrap();
    assert_eq!(scene.node(a).name, "Box");
    assert_eq!(scene.node(copy).name, "Box copy");
    let again = scene.duplicate(a).unwrap();
    assert_eq!(scene.node(again).name, "Box copy 2");

    // Duplicating a duplicate is how a row of something actually gets laid
    // out, and each one is of the one just made. Without trimming the
    // suffix the fourth press of Ctrl+D reads "Box copy copy copy copy".
    let mut chained = copy;
    for expected in ["Box copy 3", "Box copy 4", "Box copy 5"] {
        chained = scene.duplicate(chained).unwrap();
        assert_eq!(scene.node(chained).name, expected);
    }
}

#[test]
pub(crate) fn a_duplicated_group_renames_what_travels_inside_it() {
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Union, root, 0);
    box_at(&mut scene, group, 0.0);
    let copy = scene.duplicate(group).unwrap();
    assert_eq!(scene.node(copy).name, "Group copy");
    let inside = scene.node(copy).children[0];
    assert_eq!(scene.node(inside).name, "Box 2");
}

#[test]
pub(crate) fn a_name_is_free_across_the_tree_not_only_among_siblings() {
    // The outliner shows every depth at once, so a "Box 2" nested in a
    // pattern is a row the user has to tell apart from a "Box 2" beside it.
    let mut scene = Scene::new();
    let root = scene.root();
    let pattern = scene.add_pattern(root, 0);
    box_at(&mut scene, pattern, 0.0);
    let beside = box_at(&mut scene, root, 0.0);
    assert_eq!(scene.node(beside).name, "Box 2");

    let names: Vec<String> = scene.ids().map(|id| scene.node(id).name.clone()).collect();
    let mut sorted = names.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), names.len(), "{names:?}");
}
