//! Bringing coincident vertices together and dropping what that leaves
//! with no area.

use super::*;
use crate::mesh::Mesh;
use crate::vec3::Vec3;
use std::collections::HashMap;

/// Merge vertices within `tol` of each other. Unlike `Mesh::weld`, which
/// buckets by rounding and so can miss a pair that straddles a bucket
/// boundary, this checks the 27 neighbouring cells and compares actual
/// distances, which is what makes the manifold check trustworthy.
pub fn weld_tolerant(mesh: &Mesh, tol: f64) -> Mesh {
    let size = tol * 2.0;
    let mut buckets: HashMap<Cell, Vec<u32>> = HashMap::new();
    let mut positions: Vec<Vec3> = Vec::with_capacity(mesh.positions.len());
    let mut remap = vec![0u32; mesh.positions.len()];

    for (i, &p) in mesh.positions.iter().enumerate() {
        let (cx, cy, cz) = cell_of(p, size);
        let mut found = None;
        'search: for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(list) = buckets.get(&(cx + dx, cy + dy, cz + dz)) {
                        for &v in list {
                            if (positions[v as usize] - p).length() <= tol {
                                found = Some(v);
                                break 'search;
                            }
                        }
                    }
                }
            }
        }
        let id = match found {
            Some(v) => v,
            None => {
                positions.push(p);
                let v = (positions.len() - 1) as u32;
                buckets.entry((cx, cy, cz)).or_default().push(v);
                v
            }
        };
        remap[i] = id;
    }

    let mut indices = Vec::with_capacity(mesh.indices.len());
    let mut tags = Vec::with_capacity(mesh.indices.len());
    for (i, t) in mesh.indices.iter().enumerate() {
        let t = [remap[t[0] as usize], remap[t[1] as usize], remap[t[2] as usize]];
        if t[0] != t[1] && t[1] != t[2] && t[0] != t[2] {
            indices.push(t);
            tags.push(mesh.tag(i));
        }
    }
    Mesh { positions, indices, tags }
}

/// Merge the ends of every edge shorter than `limit`, and drop the triangles
/// that collapse to a line as a result.
///
/// The weld above merges points that are within a tolerance of *each other*;
/// this deals with the pair that ends up just outside it. A chain of booleans
/// puts two copies of the same physical point a few microns apart -- far enough
/// to survive the weld, close enough that the triangle between them is a
/// slither with no surface -- and there is no tolerance that separates that
/// case from a real feature by distance alone, because the two are the same
/// distance apart.
///
/// Collapsing, rather than deleting. A deleted sliver leaves its neighbours'
/// edges with nothing on the other side, which is a hole where there was a
/// seam; collapsing one end onto the other retargets those neighbours instead,
/// and the surface stays closed. Nothing moves further than `limit`.
pub(crate) fn collapse_short_edges(mesh: Mesh, limit: f64) -> Mesh {
    let mut union: Vec<u32> = (0..mesh.positions.len() as u32).collect();
    fn root(union: &mut [u32], mut i: u32) -> u32 {
        while union[i as usize] != i {
            union[i as usize] = union[union[i as usize] as usize];
            i = union[i as usize];
        }
        i
    }
    for t in &mesh.indices {
        for e in 0..3 {
            let (a, b) = (root(&mut union, t[e]), root(&mut union, t[(e + 1) % 3]));
            if a == b {
                continue;
            }
            if (mesh.positions[a as usize] - mesh.positions[b as usize]).length() < limit {
                // The lower index survives, so the result does not depend on
                // the order the triangles happen to be in.
                let (keep, gone) = if a < b { (a, b) } else { (b, a) };
                union[gone as usize] = keep;
            }
        }
    }
    let mut indices = Vec::with_capacity(mesh.indices.len());
    let mut tags = Vec::with_capacity(mesh.indices.len());
    for (i, t) in mesh.indices.iter().enumerate() {
        let t = [root(&mut union, t[0]), root(&mut union, t[1]), root(&mut union, t[2])];
        if t[0] != t[1] && t[1] != t[2] && t[0] != t[2] {
            indices.push(t);
            tags.push(mesh.tag(i));
        }
    }
    Mesh { positions: mesh.positions, indices, tags }
}

/// Drop triangles thinner than `tol`, measured as the smallest distance from a
/// vertex to the opposite edge. Welding has already merged everything closer
/// together than `tol`, so such a triangle is geometrically indistinguishable
/// from a line segment: it contributes no surface, but its three edges are
/// counted by the manifold check and by slicers. They arise wherever a face
/// carrying collinear T-junction vertices gets fan-triangulated.
pub(crate) fn drop_slivers(mesh: Mesh, tol: f64) -> Mesh {
    let pos = &mesh.positions;
    let mut indices: Vec<[u32; 3]> = Vec::with_capacity(mesh.indices.len());
    let mut tags: Vec<u32> = Vec::with_capacity(mesh.indices.len());
    for (i, t) in mesh.indices.iter().enumerate() {
        let (a, b, c) = (pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]);
        let twice_area = (b - a).cross(c - a).length();
        let longest = (b - a).length().max((c - b).length()).max((a - c).length());
        if longest > tol && twice_area / longest > tol {
            indices.push(*t);
            tags.push(mesh.tag(i));
        }
    }
    Mesh { positions: mesh.positions, indices, tags }
}
