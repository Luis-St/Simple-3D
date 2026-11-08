//! The working copy's caches, updated dab by dab, against the same built afresh.

use super::*;
use simple3d_geom::primitives::ellipsoid_mesh;
use simple3d_geom::topology::vertex_normals;

fn session(segments: u32) -> Sculpt {
    let mesh = Arc::new(MeshData::new(ellipsoid_mesh(20.0, 20.0, 20.0, segments)));
    let placement = Xform::from_pos_rot_scale(Vec3::new(5.0, 0.0, 2.0), Vec3::new(10.0, 0.0, 30.0), Vec3::ONE);
    let mut sculpt = Sculpt::new(1, mesh, placement, None);
    sculpt.begin();
    sculpt
}

/// A few dabs pressing bumps into the sphere along its normals.
fn stroke(sculpt: &mut Sculpt) {
    for dab in 0..6u32 {
        let centre = sculpt.renderable.mesh.positions[(dab * 97) as usize % sculpt.normals.len()];
        let moves: Vec<(u32, Vec3)> = (0..sculpt.normals.len() as u32)
            .filter_map(|v| {
                let at = sculpt.renderable.mesh.positions[v as usize];
                let d = (at - centre).length();
                (d < 6.0).then(|| (v, at + sculpt.normals[v as usize] * (2.0 * (1.0 - d / 6.0))))
            })
            .collect();
        sculpt.move_vertices(&moves);
    }
}

#[test]
fn a_refit_tree_answers_like_one_built_again() {
    let mut sculpt = session(96);
    assert!(sculpt.renderable.mesh.indices.len() > crate::pick::MIN_TRIANGLES);
    stroke(&mut sculpt);
    let rebuilt = Bvh::build(&sculpt.renderable.mesh);
    let mesh = &sculpt.renderable.mesh;
    let (mut hits, mut seed) = (0, 12345u64);
    let mut next = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    };
    for _ in 0..2_000 {
        let origin = Vec3::new(next(), next(), next()) * 60.0;
        let towards = Vec3::new(next(), next(), next()) * 10.0 + Vec3::new(5.0, 0.0, 2.0);
        let dir = (towards - origin).normalized();
        let refit = sculpt.ray(origin, dir);
        assert_eq!(refit, rebuilt.nearest_triangle(mesh, origin, dir));
        hits += refit.is_some() as usize;
    }
    assert!(hits > 500, "too few rays hit to say anything ({hits})");
}

#[test]
fn normals_kept_up_dab_by_dab_match_a_copy_made_afresh() {
    let mut sculpt = session(48);
    stroke(&mut sculpt);
    let fresh = Renderable::prepare_welded(sculpt.renderable.mesh.clone(), false, &sculpt.smooth, Default::default());
    let close = |a: Vec3, b: Vec3| (a - b).length() < 1e-9;
    assert!(sculpt.renderable.normals.iter().zip(&fresh.normals).all(|(&a, &b)| close(a, b)));
    let corners = sculpt.renderable.corner_normals.iter().zip(&fresh.corner_normals);
    assert!(corners.into_iter().all(|(a, b)| (0..3).all(|k| close(a[k], b[k]))));
    let vertex = vertex_normals(&sculpt.renderable.mesh);
    assert!(sculpt.normals.iter().zip(&vertex).all(|(&a, &b)| close(a, b)));
    assert!(sculpt.renderable.reach >= fresh.reach);
}

#[test]
fn every_change_is_listed_for_the_card() {
    let mut sculpt = session(48);
    let before = sculpt.renderable.mesh.positions.clone();
    let corners = sculpt.renderable.corner_normals.clone();
    stroke(&mut sculpt);
    let edits = &sculpt.edits;
    let listed = |ranges: &[std::ops::Range<u32>], at: usize| ranges.iter().any(|range| range.contains(&(at as u32)));
    for (at, (a, b)) in before.iter().zip(&sculpt.renderable.mesh.positions).enumerate() {
        assert!(a == b || listed(&edits.vertices, at), "vertex {at} moved unlisted");
    }
    for (at, (a, b)) in corners.iter().zip(&sculpt.renderable.corner_normals).enumerate() {
        assert!(a == b || listed(&edits.triangles, at), "triangle {at} turned unlisted");
    }
    assert_eq!(edits.revision - edits.since, 6, "one revision per dab");
}

#[test]
fn restoring_puts_every_vertex_and_normal_back() {
    let mut sculpt = session(48);
    let (positions, corners) = (sculpt.renderable.mesh.positions.clone(), sculpt.renderable.corner_normals.clone());
    stroke(&mut sculpt);
    sculpt.restore();
    assert_eq!(sculpt.renderable.mesh.positions, positions);
    let close = |a: Vec3, b: Vec3| (a - b).length() < 1e-12;
    assert!(sculpt.renderable.corner_normals.iter().zip(&corners).all(|(a, b)| (0..3).all(|k| close(a[k], b[k]))));
}
