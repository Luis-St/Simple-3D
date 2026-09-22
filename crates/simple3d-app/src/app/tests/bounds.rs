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
    assert!(measured.is_some());
    assert_eq!(app.selection_bounds(), measured);
}
