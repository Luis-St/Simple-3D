//! Groups, anchors and the frames they put their children in.

use super::*;
use crate::scene::{Anchor, GroupOp, Scene};
use simple3d_geom::Vec3;

#[test]
pub(crate) fn nested_groups_move_as_one() {
    // Spec acceptance criterion 8.
    let mut scene = Scene::new();
    let root = scene.root();
    let outer = scene.add_group(GroupOp::Union, root, 0);
    let inner = scene.add_group(GroupOp::Union, outer, 0);
    plate(&mut scene, inner);

    let mut evaluator = Evaluator::new();
    let before = evaluator.evaluate(&scene, &Cancel::new());
    let (lo_before, _) = before.mesh.bounds().unwrap();
    scene.get_mut(outer).unwrap().position = Vec3::new(100.0, 5.0, -3.0);
    let after = evaluator.evaluate(&scene, &Cancel::new());
    let (lo_after, _) = after.mesh.bounds().unwrap();

    assert_eq!(size(&before.mesh), size(&after.mesh));
    let moved = lo_after - lo_before;
    assert!((moved.x - 100.0).abs() < 1e-9 && (moved.y - 5.0).abs() < 1e-9 && (moved.z + 3.0).abs() < 1e-9);
}

#[test]
pub(crate) fn the_base_anchor_moves_the_origin_not_the_shape() {
    // Spec acceptance criterion 7.
    let mut scene = Scene::new();
    let root = scene.root();
    let id = plate(&mut scene, root);
    let mut evaluator = Evaluator::new();
    let centred = evaluator.evaluate(&scene, &Cancel::new());
    scene.get_mut(id).unwrap().anchor = Anchor::Base;
    let based = evaluator.evaluate(&scene, &Cancel::new());

    assert_eq!(size(&centred.mesh), size(&based.mesh));
    assert!((based.mesh.bounds().unwrap().0.z).abs() < 1e-9);
    assert!((centred.mesh.bounds().unwrap().0.z + 2.0).abs() < 1e-9);
}

#[test]
pub(crate) fn per_node_meshes_land_in_world_space() {
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Union, root, 0);
    scene.get_mut(group).unwrap().position = Vec3::new(50.0, 0.0, 0.0);
    let id = plate(&mut scene, group);
    scene.get_mut(id).unwrap().position = Vec3::new(0.0, 10.0, 0.0);

    let out = Evaluator::new().evaluate(&scene, &Cancel::new());
    let mesh = out.node_meshes.get(&id).expect("primitive mesh missing");
    let (lo, hi) = mesh.bounds().unwrap();
    let centre = (lo + hi) * 0.5;
    assert!((centre.x - 50.0).abs() < 1e-9 && (centre.y - 10.0).abs() < 1e-9, "{centre:?}");
}

#[test]
pub(crate) fn a_nodes_frame_places_its_origin_and_orients_its_axes() {
    // What a manipulator handle relies on: `node_frames[id]` is the *parent*
    // frame, so the node's origin is `frame.point(node.position)` and its
    // own axes come from composing its rotation on top.
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Union, root, 0);
    scene.get_mut(group).unwrap().position = Vec3::new(50.0, 0.0, 0.0);
    scene.get_mut(group).unwrap().rotation = Vec3::new(0.0, 0.0, 90.0);
    let id = plate(&mut scene, group);
    scene.get_mut(id).unwrap().position = Vec3::new(10.0, 0.0, 0.0);

    let out = Evaluator::new().evaluate(&scene, &Cancel::new());
    let frame = out.node_frames[&id];
    let origin = frame.point(scene.node(id).position);
    // The group's 90-degree Z rotation turns the child's local +X into +Y.
    assert!((origin - Vec3::new(50.0, 10.0, 0.0)).length() < 1e-9, "{origin:?}");

    // And the world-space mesh agrees with that origin.
    let (lo, hi) = out.node_meshes[&id].bounds().unwrap();
    let centre = (lo + hi) * 0.5;
    assert!((centre - origin).length() < 1e-9, "{centre:?} vs {origin:?}");

    // Turning a world-space drag back into parent-frame coordinates.
    let dragged_to = Vec3::new(50.0, 25.0, 0.0);
    let new_position = frame.inverse().point(dragged_to);
    assert!((new_position - Vec3::new(25.0, 0.0, 0.0)).length() < 1e-9, "{new_position:?}");
}

#[test]
pub(crate) fn local_bounds_follow_the_anchor_and_ignore_the_transform() {
    let mut scene = Scene::new();
    let root = scene.root();
    let id = plate(&mut scene, root);
    scene.get_mut(id).unwrap().position = Vec3::new(100.0, 200.0, 300.0);
    scene.get_mut(id).unwrap().rotation = Vec3::new(0.0, 90.0, 0.0);

    let mut evaluator = Evaluator::new();
    let centred = evaluator.evaluate(&scene, &Cancel::new());
    let (lo, hi) = centred.node_local_bounds[&id];
    assert!((hi - lo - Vec3::new(40.0, 20.0, 4.0)).length() < 1e-9, "{:?}", hi - lo);
    assert!((lo.z + 2.0).abs() < 1e-9, "centre anchor should straddle zero, got {}", lo.z);

    scene.get_mut(id).unwrap().anchor = Anchor::Base;
    let based = evaluator.evaluate(&scene, &Cancel::new());
    let (lo, hi) = based.node_local_bounds[&id];
    assert!(lo.z.abs() < 1e-9, "base anchor should put the local minimum at zero, got {}", lo.z);
    assert!((hi - lo - Vec3::new(40.0, 20.0, 4.0)).length() < 1e-9);
}

#[test]
pub(crate) fn a_group_frame_carries_its_ancestors_anchor_shift() {
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Union, root, 0);
    scene.get_mut(group).unwrap().anchor = Anchor::Base;
    let id = plate(&mut scene, group);

    let out = Evaluator::new().evaluate(&scene, &Cancel::new());
    // The group's base anchor lifted its contents by half the plate's
    // thickness, and the child's frame has to know that or its handles would
    // sit below the geometry.
    let origin = out.node_frames[&id].point(scene.node(id).position);
    assert!((origin.z - 2.0).abs() < 1e-9, "{origin:?}");
    let (lo, _) = out.node_meshes[&id].bounds().unwrap();
    assert!(lo.z.abs() < 1e-9, "{lo:?}");
}

#[test]
pub(crate) fn a_group_measures_the_assembly_it_evaluates_to() {
    // A group owns no mesh of its own, so the property editor used to report
    // "no geometry yet" for every group in the scene, forever. Its measured
    // size is the size of what it evaluates to, in world space.
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Difference, root, 0);
    scene.get_mut(group).unwrap().position = Vec3::new(100.0, 0.0, 0.0);
    plate(&mut scene, group);
    let hole = cylinder(&mut scene, group, 6.0, 20.0);
    scene.get_mut(hole).unwrap().position = Vec3::new(-8.0, 0.0, 0.0);

    let out = Evaluator::new().evaluate(&scene, &Cancel::new());
    let (lo, hi) = out.node_world_bounds[&group];
    assert!((hi - lo - Vec3::new(40.0, 20.0, 4.0)).length() < 1e-9, "{:?}", hi - lo);
    // In world space, so the group's own position is included.
    assert!(((lo.x + hi.x) / 2.0 - 100.0).abs() < 1e-9, "{lo:?}");
    // And the root, which is a group too.
    assert!(out.node_world_bounds.contains_key(&root));
}

#[test]
pub(crate) fn a_rotated_group_is_measured_over_its_geometry_not_its_box() {
    // Transporting a group's local box by rotating its eight corners would
    // report a 40mm plate turned 45 degrees as 42mm across -- the box's
    // diagonal, not the plate's. The measurement has to come from the
    // geometry.
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Union, root, 0);
    plate(&mut scene, group);
    scene.get_mut(group).unwrap().rotation = Vec3::new(0.0, 0.0, 90.0);

    let out = Evaluator::new().evaluate(&scene, &Cancel::new());
    let (lo, hi) = out.node_world_bounds[&group];
    // Turned a quarter turn about Z, the 40 x 20 plate measures 20 x 40.
    assert!((hi - lo - Vec3::new(20.0, 40.0, 4.0)).length() < 1e-9, "{:?}", hi - lo);
}
