//! A node's mirror image (issue 122).

use super::*;
use crate::xform::Xform;

fn close(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < 1e-7
}

/// Where a point of the node's own body lands in the world, before and after.
fn lands(node: &Node, parent: Xform, p: Vec3) -> Vec3 {
    parent.compose(&Xform::from_pos_rot_scale(node.position, node.rotation, Node::sane_scale(node.scale))).point(p)
}

#[test]
pub(crate) fn a_box_mirrored_straight_across_keeps_its_turn_and_flips_one_axis() {
    let mut scene = Scene::new();
    let root = scene.root();
    let id = box_at(&mut scene, root, 10.0);
    let mirrored = scene.node(id).mirrored(Xform::IDENTITY, Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0));
    assert!(close(mirrored.position, Vec3::new(-10.0, 0.0, 0.0)), "{mirrored:?}");
    assert_eq!(mirrored.rotation, Vec3::ZERO);
    assert_eq!(mirrored.scale, Vec3::new(-1.0, 1.0, 1.0));
}

#[test]
pub(crate) fn every_point_lands_on_its_reflection_and_twice_comes_back() {
    let mut scene = Scene::new();
    let root = scene.root();
    let id = box_at(&mut scene, root, 0.0);
    {
        let node = scene.get_mut(id).unwrap();
        node.position = Vec3::new(4.0, -3.0, 7.0);
        node.rotation = Vec3::new(20.0, 35.0, 70.0);
        node.scale = Vec3::new(2.0, 0.5, 1.5);
    }
    let parent = Xform::from_pos_rot_scale(Vec3::new(1.0, 2.0, 3.0), Vec3::new(0.0, 0.0, 30.0), Vec3::splat(2.0));
    let (point, normal) = (Vec3::new(5.0, 1.0, 0.0), Vec3::new(1.0, 2.0, -0.5));
    let mirror = Xform::reflection(point, normal);
    let before = scene.node(id).clone();
    let once = before.mirrored(parent, point, normal);
    let mut after = before.clone();
    (after.position, after.rotation, after.scale) = (once.position, once.rotation, once.scale);
    for p in [Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 2.0, -1.0), Vec3::new(3.0, -1.0, 2.0)] {
        let want = mirror.point(lands(&before, parent, p));
        assert!(close(lands(&after, parent, p), want), "{p:?}: {:?} vs {want:?}", lands(&after, parent, p));
    }
    assert!(Xform::from_pos_rot_scale(once.position, once.rotation, once.scale).mirrors());
    let twice = after.mirrored(parent, point, normal);
    let mut back = after.clone();
    (back.position, back.rotation, back.scale) = (twice.position, twice.rotation, twice.scale);
    for p in [Vec3::new(1.0, 0.0, 0.0), Vec3::new(3.0, -1.0, 2.0)] {
        assert!(close(lands(&back, parent, p), lands(&before, parent, p)), "mirroring twice did not come back");
    }
}
