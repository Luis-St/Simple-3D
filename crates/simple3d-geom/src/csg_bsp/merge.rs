//! Putting split faces back together, so a boolean does not leave a mesh fanned into slivers.

use super::*;
use crate::mesh::Mesh;
use crate::vec3::Vec3;

/// Merge a plane group's internal triangulation back into one polygon by dropping shared edges and
/// chaining the rest into one loop. BSP-CSG clips whole polygons, so a face pre-split by an arbitrary
/// diagonal gives neighbours mismatched T-vertices. `None` if the group is not one simple loop (e.g. a
/// face with a hole); the caller then uses the triangles.
pub(crate) fn try_merge_group(mesh: &Mesh, tri_idxs: &[usize], plane: Plane, tag: u32) -> Option<Polygon> {
    // BTreeMap, not HashMap: `start` depends on iteration order, and HashMap's is randomised, breaking
    // deterministic evaluation (spec section 5.2).
    use std::collections::{BTreeMap, BTreeSet};
    let mut edge_count: BTreeMap<((i64, i64, i64), (i64, i64, i64)), i32> = BTreeMap::new();
    let mut pos_of: BTreeMap<(i64, i64, i64), Vec3> = BTreeMap::new();
    for &ti in tri_idxs {
        let t = mesh.indices[ti];
        let pts = mesh.corners(t);
        for k in 0..3 {
            let (a, b) = (pts[k], pts[(k + 1) % 3]);
            let (ka, kb) = (pos_key(a), pos_key(b));
            pos_of.insert(ka, a);
            pos_of.insert(kb, b);
            *edge_count.entry((ka, kb)).or_insert(0) += 1;
        }
    }
    let mut next: BTreeMap<(i64, i64, i64), (i64, i64, i64)> = BTreeMap::new();
    for (&(ka, kb), &c) in edge_count.iter() {
        let rev = edge_count.get(&(kb, ka)).copied().unwrap_or(0);
        if c == 1 && rev == 0 {
            if next.insert(ka, kb).is_some() {
                return None; // non-simple boundary (a vertex used by >1 boundary edge)
            }
        } else if c != rev {
            return None; // inconsistent local triangulation; don't guess
        }
    }
    if next.is_empty() {
        return None;
    }
    let start = *next.keys().next().unwrap();
    let mut verts = Vec::new();
    let mut cur = start;
    let mut visited = BTreeSet::new();
    loop {
        if !visited.insert(cur) {
            return None;
        }
        verts.push(pos_of[&cur]);
        cur = *next.get(&cur)?;
        if cur == start {
            break;
        }
    }
    if verts.len() != next.len() {
        return None; // boundary has more than one loop (e.g. a face with a hole)
    }
    if !is_convex_loop(&verts, &plane) {
        // Splitting and fan triangulation assume convexity, so a concave merged face falls back to triangles.
        return None;
    }
    Some(Polygon::new(verts, plane, tag))
}

/// Whether the loop turns the same way at every vertex along the plane normal; collinear vertices
/// are tolerated.
pub(crate) fn is_convex_loop(verts: &[Vec3], plane: &Plane) -> bool {
    let n = verts.len();
    if n < 3 {
        return false;
    }
    let mut sign = 0.0f64;
    for i in 0..n {
        let a = verts[i];
        let b = verts[(i + 1) % n];
        let c = verts[(i + 2) % n];
        let turn = (b - a).cross(c - b).dot(plane.normal);
        if turn.abs() < 1e-12 {
            continue;
        }
        if sign == 0.0 {
            sign = turn.signum();
        } else if turn.signum() != sign {
            return false;
        }
    }
    true
}
