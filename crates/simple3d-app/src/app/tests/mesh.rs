//! Converting to a stored mesh, and joining pieces back together.

use super::*;
use simple3d_core::keymap::Command;
use simple3d_core::primitive::{ParamValue, ParamsExt};
use simple3d_core::scene::GroupOp;
use simple3d_geom::Vec3;

// -- the bodies and commands issue 83 added ------------------------------

#[test]
pub(crate) fn converting_to_a_mesh_keeps_the_node_and_loses_the_recipe() {
    let mut app = headless_app();
    let root = app.scene.root();
    let group = app.scene.add_group(GroupOp::Difference, root, 0);
    app.scene.add_primitive("box", group, 0).unwrap();
    let hole = app.scene.add_primitive("cylinder", group, 1).unwrap();
    app.scene.get_mut(hole).unwrap().position = Vec3::new(4.0, 0.0, 0.0);
    app.scene.get_mut(group).unwrap().name = "Drilled".into();
    app.select_only(group);
    app.reevaluate_for_test();

    app.run(Command::ConvertToMesh);
    assert!(app.scene.node(group).is_mesh(), "it is still a group");
    assert_eq!(app.scene.node(group).name, "Drilled", "the node was replaced rather than converted");
    assert!(app.scene.node(group).children.is_empty(), "the operands outlived the body they made");
    assert!(app.scene.node(group).mesh().unwrap().triangle_count() > 0);
    assert_eq!(app.selection, vec![group], "the converted node should stay selected");

    // And it is one undo step that puts the whole thing back.
    app.run(Command::Undo);
    assert!(app.scene.node(group).is_group());
    assert_eq!(app.scene.node(group).children.len(), 2);
}

#[test]
pub(crate) fn converting_something_that_is_already_a_mesh_says_so_rather_than_working() {
    let mut app = headless_app();
    let id = app.primary().unwrap();
    app.run(Command::ConvertToMesh);
    let before = app.history.undo_len();
    app.run(Command::ConvertToMesh);
    assert_eq!(app.history.undo_len(), before, "converting a mesh recorded an undo step");
    let _ = id;
}

#[test]
pub(crate) fn joining_the_pieces_back_together_brings_the_original_object_back() {
    // The other half of issue 82: a split is reversible, and what comes back
    // is the recipe -- the difference with both its operands and their
    // parameters -- not the triangles the pieces are.
    let mut app = headless_app();
    let root = app.scene.root();
    let group = app.scene.add_group(GroupOp::Difference, root, 0);
    app.scene.get_mut(group).unwrap().name = "Cut plate".into();
    let plate = app.scene.add_primitive("box", group, 0).unwrap();
    app.scene.get_mut(plate).unwrap().params_mut().unwrap().insert("width".into(), ParamValue::Length(80.0));
    let knife = app.scene.add_primitive("box", group, 1).unwrap();
    {
        let node = app.scene.get_mut(knife).unwrap();
        node.params_mut().unwrap().insert("width".into(), ParamValue::Length(6.0));
        node.params_mut().unwrap().insert("depth".into(), ParamValue::Length(200.0));
        node.params_mut().unwrap().insert("height".into(), ParamValue::Length(200.0));
    }
    app.select_only(group);
    app.reevaluate_for_test();
    let before = app.evaluated.mesh.bounds().unwrap();

    split_with(&mut app, simple3d_geom::tiling::Tiling { size: 40.0, ..Default::default() });
    let split = app.primary().expect("the split is selected");
    assert!(app.scene.node(split).is_split());
    assert!(app.scene.node(split).children.len() >= 2, "the cut left the shape in one piece");

    app.run(Command::Rejoin);
    let back = app.primary().expect("the restored object is selected");
    assert_eq!(app.scene.node(back).name, "Cut plate");
    assert_eq!(app.scene.node(back).group_op(), Some(GroupOp::Difference), "it came back as something else");
    assert_eq!(app.scene.node(back).children.len(), 2, "the operands did not come back");
    let sizes: Vec<f64> =
        app.scene.node(back).children.iter().map(|&c| app.scene.node(c).params().unwrap().num("width")).collect();
    assert_eq!(sizes, vec![80.0, 6.0], "the operands came back with different dimensions");
    app.reevaluate_for_test();
    let after = app.evaluated.mesh.bounds().unwrap();
    assert!((before.0 - after.0).length() < 1e-3, "the shape moved: {before:?} -> {after:?}");
    assert!((before.1 - after.1).length() < 1e-3, "the shape moved: {before:?} -> {after:?}");
}

#[test]
pub(crate) fn the_pieces_can_be_moved_as_one_and_joined_back_where_they_now_stand() {
    // Moving the split moves the object that comes out of it: the transform
    // belongs to the node standing in the tree, not to the recipe it holds.
    let mut app = headless_app();
    let root = app.scene.root();
    let group = app.scene.add_group(GroupOp::Union, root, 0);
    for i in 0..2 {
        let id = app.scene.add_primitive("box", group, i).unwrap();
        app.scene.get_mut(id).unwrap().position = Vec3::new(i as f64 * 60.0, 0.0, 0.0);
    }
    app.select_only(group);
    app.reevaluate_for_test();

    split_with(&mut app, simple3d_geom::tiling::Tiling { size: 40.0, ..Default::default() });
    let split = app.primary().unwrap();
    app.scene.get_mut(split).unwrap().position = Vec3::new(0.0, 0.0, 25.0);
    app.scene.get_mut(split).unwrap().name = "Renamed".into();
    app.reevaluate_for_test();
    let moved = app.evaluated.mesh.bounds().unwrap();

    app.run(Command::Rejoin);
    let back = app.primary().unwrap();
    assert_eq!(app.scene.node(back).position, Vec3::new(0.0, 0.0, 25.0), "the object went back to where it was");
    assert_eq!(app.scene.node(back).name, "Renamed", "the name the row now carries was not kept");
    app.reevaluate_for_test();
    let after = app.evaluated.mesh.bounds().unwrap();
    assert!((moved.0 - after.0).length() < 1e-3, "joining moved the shape: {moved:?} -> {after:?}");
    assert!((moved.1 - after.1).length() < 1e-3, "joining moved the shape: {moved:?} -> {after:?}");
}
