//! Reassembling into one shape: the mesh's node becomes it, with no group around it.

use super::reassemble::open_with;
use super::*;
use simple3d_geom::reassemble::Reassemble;
use simple3d_geom::Vec3;

/// Regression: one recognised sphere came back as a group "Ball" holding "Ball 2". With one shape
/// and nothing else, the mesh's node becomes that shape, keeping its name and colour.
#[test]
fn a_single_shape_replaces_the_mesh_without_a_group() {
    let mut app = app_in(temp_config_dir("reassemble-single"));
    let root = app.scene.root();
    let mesh = simple3d_geom::primitives::ellipsoid_mesh(30.0, 30.0, 30.0, 48);
    let id = app.scene.add_mesh("Ball", simple3d_core::mesh_data::MeshData::new(mesh), root, 0);
    let node = app.scene.get_mut(id).unwrap();
    node.position = Vec3::new(12.0, -4.0, 3.0);
    node.rotation = Vec3::new(0.0, 0.0, 30.0);
    node.colour = Some(simple3d_core::scene::Colour([200, 40, 40]));
    app.select_only(id);
    app.reevaluate_for_test();
    let before = app.evaluated.mesh.clone();

    open_with(&mut app, Reassemble::default());
    app.apply_reassemble();
    app.reevaluate_for_test();

    let node = app.scene.node(id);
    assert!(node.children.is_empty() && !node.is_group(), "the sphere came back wrapped in a group");
    assert_eq!(node.spec().map(|spec| spec.type_id), Some("sphere"));
    assert_eq!((node.name.as_str(), node.colour), ("Ball", Some(simple3d_core::scene::Colour([200, 40, 40]))));
    assert_eq!(app.scene.node(root).children, vec![id]);
    let moved = simple3d_geom::simplify::measure::furthest_from(&app.evaluated.mesh.positions, &before);
    assert!(moved < 1e-2, "the sphere came back {moved} mm from where the triangles were");
}

/// A fit turned inside a turned node lands where its triangles were.
#[test]
fn a_turned_box_in_a_turned_node_lands_where_its_triangles_were() {
    let mut app = app_in(temp_config_dir("reassemble-single-turned"));
    let root = app.scene.root();
    let turn = simple3d_core::xform::Xform::from_pos_rot(Vec3::new(5.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 30.0));
    let mut mesh = simple3d_geom::primitives::box_mesh(30.0, 10.0, 6.0);
    for p in &mut mesh.positions {
        *p = turn.point(*p);
    }
    let id = app.scene.add_mesh("Bar", simple3d_core::mesh_data::MeshData::new(mesh), root, 0);
    let node = app.scene.get_mut(id).unwrap();
    node.rotation = Vec3::new(20.0, 0.0, 0.0);
    node.scale = Vec3::splat(2.0);
    app.select_only(id);
    app.reevaluate_for_test();
    let before = app.evaluated.mesh.clone();

    open_with(&mut app, Reassemble::default());
    app.apply_reassemble();
    app.reevaluate_for_test();

    assert_eq!(app.scene.node(id).spec().map(|spec| spec.type_id), Some("box"), "not one box in place of the mesh");
    let moved = simple3d_geom::simplify::measure::furthest_from(&app.evaluated.mesh.positions, &before);
    assert!(moved < 1e-3, "the box came back {moved} mm from where the triangles were");
}
