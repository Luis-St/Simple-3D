//! The features a body offers.

use super::*;
use simple3d_geom::{Mesh, Vec3};
use std::collections::HashMap;

/// Every snap feature of a mesh, vertices then edge midpoints then face centres, so index
/// tie-breaks prefer exact kinds. Welded first; only crease or boundary edges count (not diagonals
/// within a flat face), only vertices they touch are corners, and each flat face reports one centre.
pub fn features_of(mesh: &Mesh) -> Vec<Feature> {
    let welded = mesh.weld();
    let n = welded.indices.len();
    if n == 0 {
        return Vec::new();
    }
    let normals: Vec<Vec3> = welded.indices.iter().map(|&t| welded.triangle_normal(t)).collect();

    // Triangles per undirected edge, to find coplanar neighbours without an O(n^2) sweep.
    let mut edge_tris: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (i, tri) in welded.indices.iter().enumerate() {
        for e in 0..3 {
            let (a, b) = (tri[e], tri[(e + 1) % 3]);
            edge_tris.entry((a.min(b), a.max(b))).or_default().push(i);
        }
    }

    // Interior to a flat face: exactly two triangles facing the same way; anything else is a real edge.
    let interior = |tris: &[usize]| tris.len() == 2 && normals[tris[0]].dot(normals[tris[1]]) > 0.9999;

    let mut features = Vec::new();
    let mut corner_indices: Vec<u32> = Vec::new();
    for (&(a, b), tris) in &edge_tris {
        if interior(tris) {
            continue;
        }
        features.push(Feature::edge(welded.positions[a as usize], welded.positions[b as usize]));
        corner_indices.push(a);
        corner_indices.push(b);
    }
    corner_indices.sort_unstable();
    corner_indices.dedup();
    // Vertices go first when the list is reordered below, so the exact kind wins screen ties.
    let mut vertices: Vec<Feature> =
        corner_indices.iter().map(|&i| Feature::point(welded.positions[i as usize], FeatureKind::Vertex)).collect();

    // Coplanar triangles joined into one face, so each face reports one centre.
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    for tris in edge_tris.values() {
        if interior(tris) {
            let (i, j) = (tris[0], tris[1]);
            let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
            if ri != rj {
                parent[ri] = rj;
            }
        }
    }
    // Area-weighted centroids, so unevenly triangulated faces report their true middle.
    let mut sums: HashMap<usize, (Vec3, f64)> = HashMap::new();
    for (i, &tri) in welded.indices.iter().enumerate() {
        let [a, b, c] = welded.corners(tri);
        let area = (b - a).cross(c - a).length() * 0.5;
        let centroid = (a + b + c) * (1.0 / 3.0);
        let root = find(&mut parent, i);
        let entry = sums.entry(root).or_insert((Vec3::ZERO, 0.0));
        entry.0 = entry.0 + centroid * area;
        entry.1 += area;
    }
    let mut centres: Vec<Feature> = sums
        .values()
        .filter(|(_, area)| *area > 1e-9)
        .map(|(sum, area)| Feature::point(*sum * (1.0 / area), FeatureKind::FaceCentre))
        .collect();

    // Deterministic order within each kind, so ties break the same way every run.
    let by_point = |a: &Feature, b: &Feature| {
        (a.point.x, a.point.y, a.point.z)
            .partial_cmp(&(b.point.x, b.point.y, b.point.z))
            .unwrap_or(std::cmp::Ordering::Equal)
    };
    vertices.sort_by(by_point);
    features.sort_by(by_point);
    centres.sort_by(by_point);

    let mut out = vertices;
    out.append(&mut features);
    out.append(&mut centres);
    out
}
