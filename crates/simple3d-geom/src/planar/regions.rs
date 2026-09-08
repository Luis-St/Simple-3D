//! Gathering coplanar triangles into regions and finding the boundary of
//! each one.

use crate::mesh::Mesh;
use crate::vec3::Vec3;
use std::collections::BTreeMap;

pub(crate) fn plane_of(mesh: &Mesh, t: [u32; 3]) -> Option<(Vec3, f64)> {
    let (a, b, c) = (mesh.positions[t[0] as usize], mesh.positions[t[1] as usize], mesh.positions[t[2] as usize]);
    let n = (b - a).cross(c - a);
    if n.length() < 1e-12 {
        return None;
    }
    let n = n.normalized();
    Some((n, n.dot(a)))
}

/// Quantised so two triangles of the same physical face land in the same group
/// despite last-bit differences in their computed normals. Deliberately
/// sign-sensitive: two faces back to back are not one region.
pub(crate) fn plane_key(normal: Vec3, w: f64) -> (i64, i64, i64, i64) {
    let s = 1_000_000.0;
    (
        (normal.x * s).round() as i64,
        (normal.y * s).round() as i64,
        (normal.z * s).round() as i64,
        (w * s).round() as i64,
    )
}

/// The region's boundary, as closed loops of vertex indices wound the same way
/// as the triangles that produced them.
///
/// An edge interior to the region is used once in each direction by the two
/// triangles sharing it; a boundary edge is used in one direction only. Returns
/// `None` if the region is not a clean set of simple loops -- a directed edge
/// used twice (the region overlaps itself), or a vertex with two outgoing
/// boundary edges (the boundary pinches there, and which way to turn is a guess).
pub(crate) fn boundary_loops(mesh: &Mesh, tris: &[usize]) -> Option<Vec<Vec<u32>>> {
    let mut used: BTreeMap<(u32, u32), u32> = BTreeMap::new();
    for &ti in tris {
        let t = mesh.indices[ti];
        for k in 0..3 {
            let count = used.entry((t[k], t[(k + 1) % 3])).or_insert(0);
            *count += 1;
            if *count > 1 {
                return None;
            }
        }
    }

    let mut next: BTreeMap<u32, u32> = BTreeMap::new();
    for &(a, b) in used.keys() {
        if used.contains_key(&(b, a)) {
            continue; // interior edge
        }
        if next.insert(a, b).is_some() {
            return None; // the boundary pinches at `a`
        }
    }
    if next.is_empty() {
        return None; // a closed surface with no boundary is not a flat region
    }

    let total = next.len();
    let mut loops: Vec<Vec<u32>> = Vec::new();
    while let Some((&start, _)) = next.iter().next() {
        let mut loop_verts = Vec::new();
        let mut current = start;
        loop {
            loop_verts.push(current);
            current = next.remove(&current)?;
            if current == start {
                break;
            }
            if loop_verts.len() > total {
                return None;
            }
        }
        if loop_verts.len() < 3 {
            return None;
        }
        loops.push(loop_verts);
    }
    Some(loops)
}
