//! A welded mesh's connected bodies and its edges with the triangles that meet at them.

use super::*;

/// Group a welded mesh's vertices into connected bodies by union-find over the triangles.
/// Past `u16::MAX` bodies they share the last number.
pub(crate) fn bodies_of(mesh: &Mesh) -> Vec<u16> {
    let mut parent: Vec<u32> = (0..mesh.positions.len() as u32).collect();
    fn find(parent: &mut [u32], mut of: u32) -> u32 {
        while parent[of as usize] != of {
            parent[of as usize] = parent[parent[of as usize] as usize];
            of = parent[of as usize];
        }
        of
    }
    for tri in &mesh.indices {
        let root = find(&mut parent, tri[0]);
        for &vertex in &tri[1..] {
            let other = find(&mut parent, vertex);
            if other != root {
                parent[other as usize] = root;
            }
        }
    }
    let mut numbers: std::collections::HashMap<u32, u16> = std::collections::HashMap::new();
    (0..mesh.positions.len() as u32)
        .map(|vertex| {
            let root = find(&mut parent, vertex);
            let next = numbers.len().min(u16::MAX as usize - 1) as u16;
            *numbers.entry(root).or_insert(next)
        })
        .collect()
}

/// Real creases plus single-triangle edges; every triangle edge would just be noise.
/// Test entry point.
#[cfg(test)]
pub fn feature_edges(mesh: &Mesh, angle_deg: f64) -> Vec<[u32; 2]> {
    let normals: Vec<Vec3> = mesh.indices.iter().map(|tri| mesh.triangle_normal(*tri)).collect();
    EdgeTable::of(mesh).feature_edges(&normals, angle_deg)
}

/// Every edge with the triangles that meet at it, in a deterministic order.
#[cfg(test)]
pub(crate) fn border_edges(mesh: &Mesh) -> Vec<BorderEdge> {
    EdgeTable::of(mesh).border_edges()
}

/// Every edge of a mesh with its triangles, grouped by the edge's lower vertex like a sparse
/// matrix row: `entries[start[v]..start[v + 1]]` holds (upper end, triangle), sorted.
///
/// Replaces a hash map that took most of a second on a 1.5M-triangle import.
pub(super) struct EdgeTable {
    start: Vec<u32>,
    /// Upper end, then triangle.
    entries: Vec<(u32, u32)>,
}

impl EdgeTable {
    pub(super) fn of(mesh: &Mesh) -> EdgeTable {
        let lower = |tri: &[u32; 3], k: usize| {
            let (a, b) = (tri[k], tri[(k + 1) % 3]);
            if a < b {
                (a, b)
            } else {
                (b, a)
            }
        };
        let mut start = vec![0u32; mesh.positions.len() + 1];
        for tri in &mesh.indices {
            for k in 0..3 {
                start[lower(tri, k).0 as usize + 1] += 1;
            }
        }
        for v in 0..mesh.positions.len() {
            start[v + 1] += start[v];
        }
        let mut cursor = start.clone();
        let mut entries = vec![(0u32, 0u32); mesh.indices.len() * 3];
        for (face, tri) in mesh.indices.iter().enumerate() {
            for k in 0..3 {
                let (a, b) = lower(tri, k);
                entries[cursor[a as usize] as usize] = (b, face as u32);
                cursor[a as usize] += 1;
            }
        }
        for v in 0..mesh.positions.len() {
            entries[start[v] as usize..start[v + 1] as usize].sort_unstable();
        }
        EdgeTable { start, entries }
    }

    /// Each edge in order, with its triangles in ascending order.
    fn for_each(&self, mut f: impl FnMut([u32; 2], &[(u32, u32)])) {
        for a in 0..self.start.len().saturating_sub(1) {
            let row = &self.entries[self.start[a] as usize..self.start[a + 1] as usize];
            for run in row.chunk_by(|x, y| x.0 == y.0) {
                f([a as u32, run[0].0], run);
            }
        }
    }

    pub(super) fn feature_edges(&self, normals: &[Vec3], angle_deg: f64) -> Vec<[u32; 2]> {
        let cos_limit = angle_deg.to_radians().cos();
        let mut edges = Vec::new();
        self.for_each(|ends, faces| {
            let keep = match faces {
                [(_, a), (_, b)] => normals[*a as usize].dot(normals[*b as usize]) < cos_limit,
                // A boundary edge or a non-manifold junction: both are worth seeing.
                _ => true,
            };
            if keep {
                edges.push(ends);
            }
        });
        edges
    }

    pub(super) fn border_edges(&self) -> Vec<BorderEdge> {
        let mut out = Vec::new();
        self.for_each(|ends, faces| {
            // Anything but two faces is always drawn; `push_selection` reads a repeated triangle as
            // "no far side".
            let pair = match faces {
                [(_, a), (_, b)] => [*a, *b],
                _ => [faces[0].1; 2],
            };
            out.push(BorderEdge { ends, faces: pair, junction: faces.len() > 2 });
        });
        out
    }
}
