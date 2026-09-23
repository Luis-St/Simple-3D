//! Which welded vertices of the scene each body is, for the GPU to leave out
//! while the body is dragged.

use super::*;
use simple3d_geom::{primitives, Mesh, Vec3};
use std::collections::BTreeMap;
use std::ops::Range;

/// `meshes` laid one after another, and the stretch of vertices each one is.
fn laid(meshes: &[Mesh]) -> (Mesh, BTreeMap<u64, Range<u32>>) {
    let mut scene = Mesh::new();
    let mut ranges = BTreeMap::new();
    for (index, mesh) in meshes.iter().enumerate() {
        let start = scene.positions.len() as u32;
        scene.append(mesh);
        ranges.insert(index as u64 + 1, start..scene.positions.len() as u32);
    }
    (scene, ranges)
}

#[test]
pub(crate) fn a_body_sharing_nothing_is_a_stretch_of_its_own() {
    let left = primitives::box_mesh(10.0, 10.0, 10.0);
    let right = primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(50.0, 0.0, 0.0));
    let (scene, ranges) = laid(&[left, right]);
    let prepared = Renderable::prepare_scene(&scene, &ranges);

    let (a, b) = (&prepared.parts[&1], &prepared.parts[&2]);
    // A box welds to its eight corners, and the two follow on from each other.
    assert_eq!((a.clone(), b.clone()), (0..8, 8..16));
    // Every triangle is one body's or the other's, never a bit of both.
    for tri in &prepared.mesh.indices {
        let inside = |range: &Range<u32>| tri.iter().filter(|&&v| range.contains(&v)).count();
        assert!(matches!((inside(a), inside(b)), (3, 0) | (0, 3)), "{tri:?} straddles the two");
    }
}

#[test]
pub(crate) fn bodies_that_touch_are_not_parts() {
    // Side by side and meeting along a face: the weld joins them where they
    // touch, so leaving one out would tear the other.
    let left = primitives::box_mesh(10.0, 10.0, 10.0);
    let right = primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(10.0, 0.0, 0.0));
    let far = primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(100.0, 0.0, 0.0));
    let (scene, ranges) = laid(&[left, right, far]);
    let prepared = Renderable::prepare_scene(&scene, &ranges);

    assert!(!prepared.parts.contains_key(&1) && !prepared.parts.contains_key(&2));
    assert!(prepared.parts.contains_key(&3));
}

#[test]
pub(crate) fn a_body_is_found_inside_the_one_it_came_in() {
    // A group of two bodies is a stretch holding both of theirs.
    let a = primitives::box_mesh(10.0, 10.0, 10.0);
    let b = primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(50.0, 0.0, 0.0));
    let (scene, mut ranges) = laid(&[a, b]);
    ranges.insert(9, 0..scene.positions.len() as u32);
    let prepared = Renderable::prepare_scene(&scene, &ranges);

    assert_eq!(prepared.parts[&9], 0..16);
    assert_eq!(prepared.parts[&1], 0..8);
}
