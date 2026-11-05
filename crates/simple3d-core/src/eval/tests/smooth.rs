//! Which surfaces an evaluation marks for smooth shading.

use super::*;
use crate::mesh_data::MeshData;
use crate::scene::GroupOp;

#[test]
pub(crate) fn only_a_mesh_body_is_smooth_and_a_hole_cut_into_it_is_part_of_it() {
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Difference, root, 0);
    let ball = scene.add_mesh(
        "Ball",
        MeshData::new(simple3d_geom::primitives::ellipsoid_mesh(20.0, 20.0, 20.0, 24)),
        group,
        0,
    );
    let hole = cylinder(&mut scene, group, 6.0, 40.0);
    let loose = plate(&mut scene, root);

    let result = Evaluator::new().evaluate(&scene, &Cancel::new());
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(result.smooth, [source_of(ball)].into());
    // A cut's wall is the inside of the body it drills (issue 114), so it is the ball's and smooth
    // too; the cutter leaves no surface of its own.
    let sources: std::collections::BTreeSet<u32> =
        (0..result.mesh.indices.len()).map(|i| result.mesh.source(i)).collect();
    assert_eq!(sources, [source_of(ball), source_of(loose)].into());
    assert!(!sources.contains(&source_of(hole)));
    assert!(!result.smooth.contains(&source_of(loose)));
}
