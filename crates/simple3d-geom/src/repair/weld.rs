//! Bringing coincident vertices together and dropping what that leaves with no area.

use super::*;
use crate::mesh::FastMap;
use crate::mesh::Mesh;
use crate::vec3::Vec3;

/// Merge vertices within `tol`. Unlike `Mesh::weld`'s rounding buckets, it checks the 27 neighbour
/// cells by real distance, so pairs straddling a bucket boundary are caught.
pub fn weld_tolerant(mesh: &Mesh, tol: f64) -> Mesh {
    let size = tol * 2.0;
    let mut buckets: FastMap<Cell, Vec<u32>> = FastMap::default();
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
    Mesh { positions, indices, tags, sources: Vec::new() }
}

/// Merge the ends of edges shorter than `limit`, dropping triangles that collapse. Chained booleans
/// leave copies of a point a few microns apart, just outside the weld; collapsing (not deleting)
/// retargets neighbours so the surface stays closed. Nothing moves further than `limit`.
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
                // The lower index survives, so the result is independent of triangle order.
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
    Mesh { positions: mesh.positions, indices, tags, sources: Vec::new() }
}

/// Drop triangles thinner than `tol` (vertex to opposite edge), which are effectively segments but
/// still count in the manifold check; they come from fanning faces with collinear T-junction vertices.
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
    Mesh { positions: mesh.positions, indices, tags, sources: Vec::new() }
}
