//! The colour a subtree carries.

use super::*;

#[test]
pub(crate) fn only_a_subtree_that_carries_a_colour_has_one_to_clear() {
    // What decides whether the Clear control is offered: a shape that
    // merely inherits a group's colour has nothing of its own to take
    // away, and a control that cannot do anything should not be live.
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Union, root, 0);
    let child = box_at(&mut scene, group, 0.0);
    assert!(!scene.subtree_is_painted(group));
    assert!(!scene.subtree_is_painted(child));

    scene.paint_subtree(group, Some(Colour([1, 2, 3])));
    assert!(scene.subtree_is_painted(group), "the group carries the colour");
    assert!(!scene.subtree_is_painted(child), "the child only inherits it");
    assert_eq!(scene.effective_colour(child), Some(Colour([1, 2, 3])));

    scene.paint_subtree(child, Some(Colour([9, 9, 9])));
    assert!(scene.subtree_is_painted(child));
    // And clearing the group takes the child's own colour with it, which is
    // what makes painting a group mean the whole group.
    scene.paint_subtree(group, None);
    assert!(!scene.subtree_is_painted(group));
    assert_eq!(scene.effective_colour(child), None);
}

#[test]
pub(crate) fn a_colour_reads_back_from_the_hex_it_is_written_as() {
    assert_eq!(Colour::from_hex("#2e9aff"), Some(Colour([0x2E, 0x9A, 0xFF])));
    assert_eq!(Colour::from_hex("2E9AFF"), Some(Colour([0x2E, 0x9A, 0xFF])));
    assert_eq!(Colour([0x2E, 0x9A, 0xFF]).to_hex(), "#2e9aff");
    for text in ["", "#", "#12345", "#1234567", "#gggggg", "nonsense"] {
        assert_eq!(Colour::from_hex(text), None, "{text:?} is not a colour");
    }
}
