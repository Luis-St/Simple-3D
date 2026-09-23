//! Which part of the scene's mesh each node's geometry is.

use super::*;
use crate::scene::{GroupOp, Scene};
use simple3d_geom::Vec3;

/// The vertices `range` covers in `mesh`, next to the node's own world mesh:
/// the two must be the same points in the same order.
fn assert_is_its_own(out: &Evaluated, id: NodeId) {
    let range = out.ranges.get(&id).unwrap_or_else(|| panic!("{id:?} has no range"));
    let own = &out.node_meshes[&id];
    let there = &out.mesh.positions[range.start as usize..range.end as usize];
    assert_eq!(there.len(), own.positions.len(), "{id:?} covers the wrong number of vertices");
    for (a, b) in there.iter().zip(&own.positions) {
        assert!((*a - *b).length() < 1e-9, "{id:?}: {a:?} is not {b:?}");
    }
}

#[test]
pub(crate) fn bodies_that_meet_nothing_are_found_where_they_landed() {
    let mut scene = Scene::new();
    let root = scene.root();
    let left = plate(&mut scene, root);
    let right = plate(&mut scene, root);
    scene.get_mut(right).unwrap().position = Vec3::new(200.0, 0.0, 0.0);
    let group = scene.add_group(GroupOp::Union, root, 2);
    scene.get_mut(group).unwrap().position = Vec3::new(0.0, 300.0, 0.0);
    let inner = plate(&mut scene, group);

    let out = Evaluator::new().evaluate(&scene, &Cancel::new());
    assert_is_its_own(&out, left);
    assert_is_its_own(&out, right);
    assert_is_its_own(&out, inner);
    // The group is its one child, and the root is the lot.
    assert_eq!(out.ranges[&group], out.ranges[&inner]);
    assert_eq!(out.ranges[&root], 0..out.mesh.positions.len() as u32);
}

#[test]
pub(crate) fn bodies_a_boolean_changed_have_no_range() {
    let mut scene = Scene::new();
    let root = scene.root();
    let a = plate(&mut scene, root);
    let b = plate(&mut scene, root);
    scene.get_mut(b).unwrap().position = Vec3::new(10.0, 0.0, 0.0);
    let far = plate(&mut scene, root);
    scene.get_mut(far).unwrap().position = Vec3::new(500.0, 0.0, 0.0);
    let cut = scene.add_group(GroupOp::Difference, root, 3);
    scene.get_mut(cut).unwrap().position = Vec3::new(0.0, 500.0, 0.0);
    let base = plate(&mut scene, cut);
    let cutter = cylinder(&mut scene, cut, 5.0, 50.0);

    let out = Evaluator::new().evaluate(&scene, &Cancel::new());
    // Two plates overlapping are unioned into one shape: neither is itself.
    assert!(!out.ranges.contains_key(&a) && !out.ranges.contains_key(&b));
    assert_is_its_own(&out, far);
    // A difference that took something away is a new shape; the cutter is
    // not in it at all.
    assert!(!out.ranges.contains_key(&base) && !out.ranges.contains_key(&cutter));
    assert!(out.ranges.contains_key(&cut));
}

#[test]
pub(crate) fn a_difference_that_misses_keeps_its_base() {
    let mut scene = Scene::new();
    let root = scene.root();
    let cut = scene.add_group(GroupOp::Difference, root, 0);
    let base = plate(&mut scene, cut);
    let cutter = cylinder(&mut scene, cut, 5.0, 50.0);
    scene.get_mut(cutter).unwrap().position = Vec3::new(300.0, 0.0, 0.0);

    let out = Evaluator::new().evaluate(&scene, &Cancel::new());
    assert_is_its_own(&out, base);
    assert!(!out.ranges.contains_key(&cutter));
}

#[test]
pub(crate) fn an_assembly_keeps_every_child_even_where_they_overlap() {
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Assembly, root, 0);
    let a = plate(&mut scene, group);
    let b = plate(&mut scene, group);
    scene.get_mut(b).unwrap().position = Vec3::new(10.0, 0.0, 0.0);

    let out = Evaluator::new().evaluate(&scene, &Cancel::new());
    assert_is_its_own(&out, a);
    assert_is_its_own(&out, b);
}

#[test]
pub(crate) fn the_ranges_come_back_from_the_cache_unchanged() {
    let mut scene = Scene::new();
    let root = scene.root();
    let still = plate(&mut scene, root);
    let moving = plate(&mut scene, root);
    scene.get_mut(moving).unwrap().position = Vec3::new(200.0, 0.0, 0.0);

    let mut evaluator = Evaluator::new();
    evaluator.evaluate(&scene, &Cancel::new());
    scene.get_mut(moving).unwrap().position = Vec3::new(0.0, 200.0, 0.0);
    let out = evaluator.evaluate(&scene, &Cancel::new());
    assert_is_its_own(&out, still);
    assert_is_its_own(&out, moving);
    assert_eq!(out.placements[&moving].point(Vec3::ZERO), Vec3::new(0.0, 200.0, 0.0));
}
