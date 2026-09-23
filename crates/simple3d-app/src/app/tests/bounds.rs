//! The sizes the interface shows every frame are read off the evaluation, not
//! measured again from the meshes each time.

use super::*;
use simple3d_core::scene::{GroupOp, NodeId};
use simple3d_geom::Vec3;

/// A group of a sphere, a box and a pattern of cylinders, evaluated.
fn assembly() -> (App, NodeId) {
    let mut app = headless_app();
    let root = app.scene.root();
    let group = app.scene.add_group(GroupOp::Union, root, 0);
    let sphere = app.scene.add_primitive("sphere", group, 0).unwrap();
    app.scene.get_mut(sphere).unwrap().position = Vec3::new(-15.0, 4.0, 2.0);
    let cube = app.scene.add_primitive("box", group, 1).unwrap();
    app.scene.get_mut(cube).unwrap().position = Vec3::new(20.0, -3.0, 7.0);
    app.scene.get_mut(cube).unwrap().rotation = Vec3::new(10.0, 25.0, 40.0);
    let pattern = app.scene.add_pattern(group, 2);
    app.scene.add_primitive("cylinder", pattern, 0).unwrap();
    app.scene.get_mut(pattern).unwrap().position = Vec3::new(0.0, 30.0, 0.0);
    app.reevaluate_for_test();
    (app, group)
}

#[test]
pub(crate) fn the_scene_and_the_selection_measure_what_their_meshes_measure() {
    let (mut app, group) = assembly();
    assert!(app.evaluated.bounds.is_some());
    assert_eq!(app.evaluated.bounds, app.evaluated.mesh.bounds());

    // Every node with a mesh of its own is recorded at exactly the box its mesh
    // has, which is what the selection's box and the outliner's size read.
    assert!(app.evaluated.node_meshes.len() >= 4, "the pattern should have a mesh of its own too");
    for (id, mesh) in &app.evaluated.node_meshes {
        assert_eq!(app.evaluated.node_world_bounds.get(id).copied(), mesh.bounds(), "node {id:?}");
    }

    app.select_only(group);
    let mut measured: Option<(Vec3, Vec3)> = None;
    for node in std::iter::once(group).chain(app.scene.descendants(group)) {
        if let Some((lo, hi)) = app.evaluated.node_meshes.get(&node).and_then(|mesh| mesh.bounds()) {
            measured = Some(measured.map_or((lo, hi), |(a, b)| (a.min(lo), b.max(hi))));
        }
    }
    // A union is the size of its parts. Read off the group's own evaluated
    // mesh now, so the last digit can differ from the parts' own.
    let ((lo, hi), (m_lo, m_hi)) = (app.selection_bounds().unwrap(), measured.unwrap());
    assert!((lo - m_lo).length() < 1e-9 && (hi - m_hi).length() < 1e-9, "{lo:?}..{hi:?} against {m_lo:?}..{m_hi:?}");
}

#[test]
pub(crate) fn a_selected_difference_is_boxed_as_what_is_left_not_with_its_cutters() {
    // The bounding box drawn round the selection took every descendant with a
    // mesh, and a difference's cutter is one: a 25 mm block drilled by a 60 mm
    // cylinder was boxed and labelled 60 mm tall, while the properties panel
    // said 25.
    let mut app = headless_app();
    let root = app.scene.root();
    let group = app.scene.add_group(GroupOp::Difference, root, 0);
    let block = app.scene.add_primitive("box", group, 0).unwrap();
    let cutter = app.scene.add_primitive("cylinder", group, 1).unwrap();
    app.scene.get_mut(cutter).unwrap().scale = Vec3::new(0.4, 0.4, 3.0);
    app.reevaluate_for_test();
    let (block_lo, block_hi) = app.evaluated.node_world_bounds[&block];
    let (cut_lo, cut_hi) = app.evaluated.node_world_bounds[&cutter];
    assert!(cut_lo.z < block_lo.z && cut_hi.z > block_hi.z, "this test needs a cutter taller than the block");

    app.select_only(group);
    assert_eq!(app.selection_bounds(), app.evaluated.node_world_bounds.get(&group).copied());
    let (lo, hi) = app.selection_bounds().expect("the difference has bounds");
    assert!((lo.z - block_lo.z).abs() < 1e-6 && (hi.z - block_hi.z).abs() < 1e-6, "boxed at {lo:?}..{hi:?}");
}
