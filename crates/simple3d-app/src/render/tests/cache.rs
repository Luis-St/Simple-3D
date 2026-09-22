//! The per-node renderables are kept for as long as their meshes are.

use super::*;
use simple3d_core::eval::{Cancel, Evaluator};
use simple3d_core::scene::Scene;
use simple3d_geom::Vec3;
use std::sync::Arc;

#[test]
pub(crate) fn a_node_that_did_not_change_is_not_prepared_again() {
    // Every evaluation during a drag used to re-prepare every selected and
    // ghosted node on the interface thread, the unchanged ones included.
    let mut scene = Scene::new();
    let root = scene.root();
    let still = scene.add_primitive("plate", root, 0).unwrap();
    let moved = scene.add_primitive("cylinder", root, 1).unwrap();
    let mut evaluator = Evaluator::new();
    let cache = RenderableCache::default();

    let first = evaluator.evaluate(&scene, &Cancel::new());
    let still_before = cache.get(&first, (still, true)).expect("the plate has a renderable");
    let moved_before = cache.get(&first, (moved, true)).expect("the cylinder has a renderable");
    // Asked again of the same evaluation: the same one.
    assert!(Arc::ptr_eq(&still_before, &cache.get(&first, (still, true)).unwrap()));

    scene.get_mut(moved).unwrap().position = Vec3::new(40.0, 0.0, 0.0);
    let second = evaluator.evaluate(&scene, &Cancel::new());
    assert!(Arc::ptr_eq(&still_before, &cache.get(&second, (still, true)).unwrap()));
    let moved_after = cache.get(&second, (moved, true)).unwrap();
    assert!(!Arc::ptr_eq(&moved_before, &moved_after), "a moved node kept the renderable of where it was");
    assert_ne!(moved_before.mesh.bounds(), moved_after.mesh.bounds());

    // The plain and the outlined renderable of one node are different things.
    assert!(cache.get(&second, (still, false)).unwrap().outline.is_empty());
    assert!(!still_before.outline.is_empty());

    // What is no longer drawn is let go of.
    cache.retain(&[(moved, true)]);
    assert!(!Arc::ptr_eq(&still_before, &cache.get(&second, (still, true)).unwrap()));
}

#[test]
pub(crate) fn the_edge_table_agrees_with_counting_every_edge() {
    // The feature edges and the outline are read off a table sorted by vertex
    // rather than a hash map of edges; this holds the two against a count done
    // the slow way, on a mesh with open boundaries and a three-way junction.
    let mut mesh = simple3d_geom::primitives::box_mesh(10.0, 10.0, 10.0);
    mesh.append(&simple3d_geom::primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(10.0, 0.0, 0.0)));
    mesh.append(&simple3d_geom::primitives::cylinder_mesh(6.0, 6.0, 4.0, 12).translated(Vec3::new(0.0, 20.0, 0.0)));
    mesh.indices.truncate(mesh.indices.len() - 5);
    mesh.tags.truncate(mesh.indices.len().min(mesh.tags.len()));
    let welded = mesh.weld();

    let mut expected: std::collections::BTreeMap<(u32, u32), Vec<u32>> = std::collections::BTreeMap::new();
    for (face, tri) in welded.indices.iter().enumerate() {
        for k in 0..3 {
            let (a, b) = (tri[k], tri[(k + 1) % 3]);
            expected.entry((a.min(b), a.max(b))).or_default().push(face as u32);
        }
    }
    let outline = border_edges(&welded);
    assert_eq!(outline.len(), expected.len());
    assert!(expected.values().any(|faces| faces.len() > 2), "the test mesh has no junction");
    assert!(expected.values().any(|faces| faces.len() == 1), "the test mesh has no open edge");
    for (edge, ((a, b), faces)) in outline.iter().zip(&expected) {
        assert_eq!(edge.ends, [*a, *b]);
        assert_eq!(edge.junction, faces.len() > 2);
        let pair = if faces.len() == 2 { [faces[0], faces[1]] } else { [faces[0]; 2] };
        assert_eq!(edge.faces, pair);
    }

    let normals: Vec<Vec3> = welded.indices.iter().map(|tri| welded.triangle_normal(*tri)).collect();
    let cos_limit = 20.0_f64.to_radians().cos();
    let creases: Vec<[u32; 2]> = expected
        .iter()
        .filter(|(_, faces)| match faces.as_slice() {
            [a, b] => normals[*a as usize].dot(normals[*b as usize]) < cos_limit,
            _ => true,
        })
        .map(|((a, b), _)| [*a, *b])
        .collect();
    assert_eq!(feature_edges(&welded, 20.0), creases);
}
