//! A scale over a whole subtree.

use super::*;
use crate::scene::{Anchor, GroupOp, Scene};
use simple3d_geom::Vec3;

#[test]
pub(crate) fn a_scale_multiplies_a_whole_subtree_and_a_base_anchor_still_stands_on_the_ground() {
    // A scale is the one thing that makes a group a proportion of what it
    // was without touching every dimension underneath it, so it has to reach
    // the children.
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Union, root, 0);
    let a = scene.add_primitive("box", group, 0).unwrap();
    scene.get_mut(a).unwrap().position = Vec3::new(10.0, 0.0, 0.0);

    let plain = Evaluator::new().evaluate(&scene, &Cancel::new());
    let (lo, hi) = plain.mesh.bounds().unwrap();

    scene.get_mut(group).unwrap().scale = Vec3::new(2.0, 2.0, 2.0);
    let scaled = Evaluator::new().evaluate(&scene, &Cancel::new());
    let (slo, shi) = scaled.mesh.bounds().unwrap();
    assert!((slo - lo * 2.0).length() < 1e-9, "{slo:?} is not twice {lo:?}");
    assert!((shi - hi * 2.0).length() < 1e-9, "{shi:?} is not twice {hi:?}");

    // Non-uniformly, on one axis only.
    scene.get_mut(group).unwrap().scale = Vec3::new(1.0, 3.0, 1.0);
    let stretched = Evaluator::new().evaluate(&scene, &Cancel::new());
    let (tlo, thi) = stretched.mesh.bounds().unwrap();
    assert!((thi.y - hi.y * 3.0).abs() < 1e-9, "the Y axis did not stretch: {thi:?}");
    assert!((thi.x - hi.x).abs() < 1e-9, "the X axis stretched too: {thi:?}");
    assert!((tlo.z - lo.z).abs() < 1e-9);

    // The anchor is applied before the scale, so a base-anchored shape is
    // still standing on z = 0 after being scaled.
    scene.get_mut(group).unwrap().scale = Vec3::ONE;
    scene.get_mut(a).unwrap().anchor = Anchor::Base;
    scene.get_mut(a).unwrap().scale = Vec3::new(1.0, 1.0, 4.0);
    let standing = Evaluator::new().evaluate(&scene, &Cancel::new());
    let (blo, bhi) = standing.mesh.bounds().unwrap();
    assert!(blo.z.abs() < 1e-9, "a base-anchored shape scaled up sank to {}", blo.z);
    assert!((bhi.z - (hi.z - lo.z) * 4.0).abs() < 1e-9, "it did not grow by the factor: {bhi:?}");
}

#[test]
pub(crate) fn a_scale_is_part_of_the_cache_key_and_of_the_world_frames() {
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Union, root, 0);
    let child = scene.add_primitive("box", group, 0).unwrap();
    scene.get_mut(child).unwrap().position = Vec3::new(10.0, 0.0, 0.0);
    let mut evaluator = Evaluator::new();
    let before = evaluator.evaluate(&scene, &Cancel::new());
    let child_centre = |out: &Evaluated| {
        let (lo, hi) = out.node_meshes[&child].bounds().unwrap();
        (lo + hi) * 0.5
    };
    assert!((child_centre(&before) - Vec3::new(10.0, 0.0, 0.0)).length() < 1e-9);

    // The same evaluator, so a stale cache entry would show up here.
    scene.get_mut(group).unwrap().scale = Vec3::new(2.0, 2.0, 2.0);
    let after = evaluator.evaluate(&scene, &Cancel::new());
    assert!(
        (child_centre(&after) - Vec3::new(20.0, 0.0, 0.0)).length() < 1e-9,
        "the child's world mesh ignored its parent's scale: {:?}",
        child_centre(&after)
    );
    assert!(after.mesh.bounds().unwrap().1.x > before.mesh.bounds().unwrap().1.x, "the scene mesh was cached");
}
