//! Capturing a node's geometry and putting it back where it stood.

use super::*;
use crate::scene::{Anchor, GroupOp, Scene};
use simple3d_geom::Vec3;

// -- the bodies issue 80 added ------------------------------------------

#[test]
pub(crate) fn baking_a_node_captures_what_it_evaluates_to_without_moving_it() {
    // Issue 80's whole correctness condition: converting must not shift the
    // shape, whatever the node's transform and anchor happen to be.
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Difference, root, 0);
    let base = plate(&mut scene, group);
    let hole = cylinder(&mut scene, group, 6.0, 40.0);
    scene.get_mut(hole).unwrap().position = Vec3::new(5.0, 0.0, 0.0);
    {
        let node = scene.get_mut(group).unwrap();
        node.position = Vec3::new(30.0, -12.0, 4.0);
        node.rotation = Vec3::new(0.0, 0.0, 35.0);
        node.scale = Vec3::new(1.5, 1.0, 2.0);
        node.anchor = Anchor::Base;
    }
    let before = Evaluator::new().evaluate(&scene, &Cancel::new()).mesh.bounds().unwrap();

    let baked = baked_mesh(&scene, group);
    assert!(baked.triangle_count() > 0);
    let mut converted = scene.clone();
    assert!(converted.convert_to_mesh(group, crate::mesh_data::MeshData::new(baked)));
    assert!(converted.node(group).is_mesh());
    assert!(converted.node(group).children.is_empty(), "the operands outlived the body they made");

    let after = Evaluator::new().evaluate(&converted, &Cancel::new()).mesh.bounds().unwrap();
    assert!((before.0 - after.0).length() < 1e-6, "the shape moved: {before:?} -> {after:?}");
    assert!((before.1 - after.1).length() < 1e-6, "the shape moved: {before:?} -> {after:?}");
    let _ = base;
}

#[test]
pub(crate) fn the_baked_placement_puts_the_geometry_back_exactly_where_the_node_stands() {
    // What a tool's preview is drawn through (issue 82). The tool works in
    // the node's own frame; if the way back out is off by an anchor, a
    // scale or an ancestor, the cells are drawn floating beside the shape
    // they are cutting rather than on it -- and the split still comes out
    // right, so nothing but the eye would catch it.
    //
    // Every one of those is turned on at once, and the answer is checked
    // against the evaluation's own world mesh rather than against a repeat
    // of the arithmetic.
    let mut scene = Scene::new();
    let root = scene.root();
    let outer = scene.add_group(GroupOp::Union, root, 0);
    {
        let node = scene.get_mut(outer).unwrap();
        node.position = Vec3::new(-14.0, 6.0, 3.0);
        node.rotation = Vec3::new(0.0, 25.0, 0.0);
    }
    let group = scene.add_group(GroupOp::Difference, outer, 0);
    plate(&mut scene, group);
    let hole = cylinder(&mut scene, group, 6.0, 40.0);
    scene.get_mut(hole).unwrap().position = Vec3::new(5.0, 0.0, 0.0);
    {
        let node = scene.get_mut(group).unwrap();
        node.position = Vec3::new(30.0, -12.0, 4.0);
        node.rotation = Vec3::new(0.0, 0.0, 35.0);
        node.scale = Vec3::new(1.5, 1.0, 2.0);
        node.anchor = Anchor::Base;
    }

    let out = Evaluator::new().evaluate(&scene, &Cancel::new());
    let parent = out.node_frames[&group];
    let (baked, placement) = baked_mesh_in_place(&scene, group, parent);
    assert!(baked.triangle_count() > 0);

    // Every baked point, put back, must land on the world mesh the
    // evaluation drew -- so the two boxes agree to the last micron.
    let placed = bounds_of(baked.positions.iter().map(|&p| placement.point(p))).expect("the baked mesh has points");
    let world = bounds_of(out.mesh.positions.iter().copied()).expect("the scene has points");
    assert!((placed.0 - world.0).length() < 1e-6, "the placement is off: {placed:?} against {world:?}");
    assert!((placed.1 - world.1).length() < 1e-6, "the placement is off: {placed:?} against {world:?}");

    // And the mesh itself is unchanged by asking for the placement with it.
    let alone = baked_mesh(&scene, group);
    assert_eq!(alone.positions, baked.positions, "asking for the placement changed the geometry");
}

#[test]
pub(crate) fn a_centre_anchored_node_is_placed_without_an_anchor_shift() {
    // The other half of the same sum: an offset applied where there is none
    // to apply would push the preview off the shape by half its height.
    let mut scene = Scene::new();
    let root = scene.root();
    let id = plate(&mut scene, root);
    scene.get_mut(id).unwrap().position = Vec3::new(3.0, 4.0, 5.0);
    let out = Evaluator::new().evaluate(&scene, &Cancel::new());
    let (_, placement) = baked_mesh_in_place(&scene, id, out.node_frames[&id]);
    assert!((placement.point(Vec3::ZERO) - Vec3::new(3.0, 4.0, 5.0)).length() < 1e-9);
}

#[test]
pub(crate) fn a_stored_mesh_is_placed_by_its_node_like_any_other_body() {
    let mut scene = Scene::new();
    let root = scene.root();
    let id = scene.add_mesh(
        "Baked",
        crate::mesh_data::MeshData::new(simple3d_geom::primitives::box_mesh(20.0, 20.0, 20.0)),
        root,
        0,
    );
    scene.get_mut(id).unwrap().position = Vec3::new(50.0, 0.0, 0.0);
    scene.get_mut(id).unwrap().scale = Vec3::new(2.0, 1.0, 1.0);

    let out = Evaluator::new().evaluate(&scene, &Cancel::new());
    let (lo, hi) = out.mesh.bounds().unwrap();
    assert!((hi.x - lo.x - 40.0).abs() < 1e-6, "the scale was not applied: {}", hi.x - lo.x);
    assert!(((lo.x + hi.x) * 0.5 - 50.0).abs() < 1e-6, "the position was not applied");
    assert!(out.node_meshes.contains_key(&id), "a mesh body has nothing to pick");
}
