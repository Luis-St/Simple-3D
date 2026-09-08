//! The tree: adding, deleting, dragging and reordering rows.

use super::*;
use simple3d_core::keymap::Command;
use simple3d_core::scene::GroupOp;

#[test]
pub(crate) fn deleting_a_group_asks_what_should_happen_to_its_children() {
    // Two readings of one word, so it is asked rather than guessed.
    let mut app = headless_app();
    let plate = app.scene.depth_first().into_iter().find(|&id| id != app.scene.root()).unwrap();
    app.select_only(plate);
    app.run(Command::Group);
    let group = app.primary().unwrap();
    assert!(app.scene.node(group).is_group());

    app.run(Command::Delete);
    assert!(app.pending_delete.is_some(), "a group with children was deleted without asking");
    assert!(app.scene.contains(group), "the model changed before the question was answered");
    assert_eq!(app.pending_delete_count(), 2, "the group and its one child");

    // Cancelling leaves everything alone.
    app.cancel_delete();
    assert!(app.pending_delete.is_none());
    assert!(app.scene.contains(group) && app.scene.contains(plate));

    // Keeping the children promotes them into the group's own place.
    app.select_only(group);
    app.run(Command::Delete);
    app.confirm_delete(true);
    assert!(!app.scene.contains(group), "the group survived");
    assert!(app.scene.contains(plate), "the child was deleted despite being kept");
    assert_eq!(app.scene.node(plate).parent, Some(app.scene.root()), "the child was not promoted");

    // And taking the children takes them.
    app.select_only(plate);
    app.run(Command::Group);
    let group = app.primary().unwrap();
    app.run(Command::Delete);
    app.confirm_delete(false);
    assert!(!app.scene.contains(group) && !app.scene.contains(plate));

    // A shape on its own is not a question, so it just goes.
    let root = app.scene.root();
    let lone = app.scene.add_primitive("sphere", root, 0).unwrap();
    app.select_only(lone);
    app.run(Command::Delete);
    assert!(app.pending_delete.is_none(), "deleting one shape asked a question it did not need to");
    assert!(!app.scene.contains(lone));
}

#[test]
pub(crate) fn moving_among_siblings_moves_everything_that_is_selected() {
    // Issue 41: with more than one node selected, only the primary used to
    // move -- which reads as the command doing nothing to the rest.
    let mut app = headless_app();
    let root = app.scene.root();
    let first = app.scene.add_primitive("box", root, 0).unwrap();
    let second = app.scene.add_primitive("box", root, 1).unwrap();
    let third = app.scene.add_primitive("box", root, 2).unwrap();
    let plate = app.scene.node(root).children[3];

    app.selection = vec![second, third];
    assert!(app.can_reorder(1), "there is a sibling below to move past");
    app.run(Command::MoveDown);
    assert_eq!(app.scene.node(root).children, vec![first, plate, second, third]);

    // And the two of them together stop at the end rather than one of them
    // running past the other.
    assert!(!app.can_reorder(1));
    app.run(Command::MoveDown);
    assert_eq!(app.scene.node(root).children, vec![first, plate, second, third]);

    app.selection = vec![first];
    assert!(!app.can_reorder(-1), "already first among its siblings");
    assert!(app.can_reorder(1));
}

#[test]
pub(crate) fn a_group_can_be_made_empty_and_have_its_operator_set_where_the_tree_is() {
    // Issues 37 and 40.
    let mut app = headless_app();
    let plate = app.primary().unwrap();
    app.add_node_at(plate, None, GroupOp::Union);
    let group = app.primary().unwrap();
    assert!(app.scene.node(group).is_group());
    assert!(app.scene.node(group).children.is_empty(), "the new group is empty");
    assert_eq!(app.scene.node(group).parent, Some(app.scene.root()), "beside the node it was made from");

    // Made *inside* a group, since a group is somewhere things can go.
    app.add_node_at(group, None, GroupOp::Union);
    let inner = app.primary().unwrap();
    assert_eq!(app.scene.node(inner).parent, Some(group));

    app.set_group_op(group, GroupOp::Difference);
    assert_eq!(app.scene.node(group).group_op(), Some(GroupOp::Difference));
}

#[test]
pub(crate) fn dragging_one_row_of_a_multi_selection_moves_the_whole_selection() {
    // Issue 43: a drag that started on a selected row carries everything
    // selected, in document order, and leaves it selected where it lands.
    let mut app = headless_app();
    let root = app.scene.root();
    let plate = app.primary().unwrap();
    let second = app.scene.add_primitive("box", root, 1).unwrap();
    let third = app.scene.add_primitive("cylinder", root, 2).unwrap();
    let group = app.scene.add_group(GroupOp::Union, root, 3);

    app.selection = vec![third, plate];
    // Grabbed on a row that is part of the selection: all of it travels,
    // in the order the tree has it rather than the order it was clicked.
    assert_eq!(app.dragged_nodes(third), vec![plate, third]);
    // Grabbed on a row that is not: that row alone, and the selection is
    // not what the gesture was about.
    assert_eq!(app.dragged_nodes(second), vec![second]);

    app.outliner_drag = Some(Carried::Rows(third));
    app.drop_target = Some(DropTarget { parent: group, index: 0, into: Some(group) });
    crate::panel_outliner::finish_drag(&mut app);
    assert_eq!(app.scene.node(group).children, vec![plate, third]);
    assert_eq!(app.scene.node(root).children, vec![second, group]);
    assert!(app.is_selected(plate) && app.is_selected(third), "the load was dropped out of the selection");
    assert_eq!(app.selection.len(), 2);

    // One undo step for the whole drag, and it puts all of it back.
    app.run(Command::Undo);
    assert_eq!(app.scene.node(root).children, vec![plate, second, third, group]);
}

#[test]
pub(crate) fn a_selection_that_holds_a_group_and_its_child_drags_as_the_group_alone() {
    let mut app = headless_app();
    let root = app.scene.root();
    let plate = app.primary().unwrap();
    let group = app.scene.add_group(GroupOp::Union, root, 1);
    let inner = app.scene.add_primitive("box", group, 0).unwrap();
    let target = app.scene.add_group(GroupOp::Union, root, 2);

    app.selection = vec![group, inner];
    assert_eq!(app.dragged_nodes(inner), vec![group], "the child was torn out of the group carrying it");

    app.outliner_drag = Some(Carried::Rows(group));
    app.drop_target = Some(DropTarget { parent: target, index: 0, into: Some(target) });
    crate::panel_outliner::finish_drag(&mut app);
    assert_eq!(app.scene.node(target).children, vec![group]);
    assert_eq!(app.scene.node(group).children, vec![inner], "the child did not travel with its group");
    assert_eq!(app.scene.node(root).children, vec![plate, target]);
}

#[test]
pub(crate) fn the_outliner_can_add_a_group_or_any_primitive_where_the_row_is() {
    // Issue 44: the row's own Add menu, which puts a new node inside the
    // group it was opened on and beside anything else.
    let mut app = headless_app();
    let plate = app.primary().unwrap();

    app.add_node_at(plate, Some("cylinder"), GroupOp::Union);
    let cylinder = app.primary().unwrap();
    assert_eq!(app.scene.node(cylinder).parent, Some(app.scene.root()), "beside the node it was added from");
    assert_eq!(app.scene.node(cylinder).spec().map(|s| s.type_id), Some("cylinder"));

    app.add_node_at(cylinder, None, GroupOp::Difference);
    let group = app.primary().unwrap();
    assert_eq!(app.scene.node(group).group_op(), Some(GroupOp::Difference));

    // Into the group, because a group is somewhere things can go.
    app.add_node_at(group, Some("box"), GroupOp::Union);
    let boxed = app.primary().unwrap();
    assert_eq!(app.scene.node(boxed).parent, Some(group));

    // Every shape the palette offers is reachable from the same menu.
    for spec in simple3d_core::primitive::REGISTRY.iter() {
        app.add_node_at(group, Some(spec.type_id), GroupOp::Union);
        let added = app.primary().unwrap();
        assert_eq!(
            app.scene.node(added).spec().map(|s| s.type_id),
            Some(spec.type_id),
            "{} could not be added from the outliner",
            spec.type_id
        );
    }
}

#[test]
pub(crate) fn a_collapsed_group_hides_its_children_and_a_selection_opens_it_again() {
    // Issue 33.
    let mut app = headless_app();
    let plate = app.primary().unwrap();
    app.run(Command::Group);
    let group = app.primary().unwrap();
    assert!(crate::panel_outliner::visible_rows(&app).contains(&plate));

    app.set_collapsed(group, true);
    let rows = crate::panel_outliner::visible_rows(&app);
    assert!(rows.contains(&group), "the group itself is still a row");
    assert!(!rows.contains(&plate), "a collapsed group still drew its children");

    // Selecting something inside it -- from the viewport, say -- has to
    // bring it back into view.
    app.select_only(plate);
    assert!(crate::panel_outliner::visible_rows(&app).contains(&plate));
}
