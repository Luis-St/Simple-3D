//! Putting split faces back together, so a boolean does not leave a mesh
//! fanned into slivers.

use super::*;
use crate::mesh::Mesh;
use crate::vec3::Vec3;

/// Undo a plane group's internal triangulation (a "fan from centre" cap, or
/// a diagonal-split quad wall) back into a single polygon, by dropping edges
/// shared by two triangles of the group and chaining what's left into one
/// boundary loop. This matters because BSP-CSG clips whole input polygons:
/// if a primitive's own flat face is fed in pre-split by an arbitrary
/// internal diagonal, a neighbouring face clipped at a slightly different
/// point along that same physical edge produces a T-vertex the strict
/// manifold check (and downstream slicers) will flag, even though the
/// surface has no real gap. Returns None (caller falls back to per-triangle
/// polygons) if the group isn't a single simple loop -- e.g. it is itself
/// the result of an earlier boolean op and legitimately has multiple
/// boundary components (a face with a hole in it).
pub(crate) fn try_merge_group(mesh: &Mesh, tri_idxs: &[usize], plane: Plane, tag: u32) -> Option<Polygon> {
    // BTreeMap, not HashMap: `start` below is picked by iteration order, and
    // `HashMap`'s is randomised per instance, which would make boolean output
    // vary between runs of identical input (spec section 5.2 requires
    // deterministic evaluation).
    use std::collections::{BTreeMap, BTreeSet};
    let mut edge_count: BTreeMap<((i64, i64, i64), (i64, i64, i64)), i32> = BTreeMap::new();
    let mut pos_of: BTreeMap<(i64, i64, i64), Vec3> = BTreeMap::new();
    for &ti in tri_idxs {
        let t = mesh.indices[ti];
        let pts = [mesh.positions[t[0] as usize], mesh.positions[t[1] as usize], mesh.positions[t[2] as usize]];
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
        // Both `split_polygon` and `polygons_to_mesh` assume convexity (the
        // latter fan-triangulates from vertex 0). A concave merged face -- an
        // L-shaped face left behind by an earlier boolean, say -- would be
        // silently mis-split and mis-triangulated, so fall back to feeding
        // this group's triangles in individually.
        return None;
    }
    Some(Polygon::new(verts, plane, tag))
}

/// True if the loop turns the same way at every vertex when viewed along the
/// plane normal. Exactly-collinear vertices (a fan triangulation's midpoints)
/// are tolerated: they are harmless for both splitting and fan-triangulation.
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
