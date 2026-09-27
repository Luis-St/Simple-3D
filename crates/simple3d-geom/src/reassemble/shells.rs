//! The separate bodies one mesh holds.

use super::*;
use crate::mesh::Mesh;

/// Break a welded mesh into its vertex-connected pieces, with their tags. Touching is not
/// connection (a pin resting in a hole is two bodies); touching bodies are grouped instead
/// ([`super::group`]). Unwelded input gives one body per triangle.
pub(super) fn shells(mesh: &Mesh, give_up: Abandon<'_>) -> Option<Vec<Mesh>> {
    if mesh.indices.is_empty() {
        return Some(Vec::new());
    }
    let mut owner = Owner::of(mesh.positions.len());
    for (i, tri) in mesh.indices.iter().enumerate() {
        // Checked often enough to abandon large meshes promptly, rarely enough not to dominate.
        if i % 4096 == 0 && give_up() {
            return None;
        }
        owner.join(tri[0] as usize, tri[1] as usize);
        owner.join(tri[1] as usize, tri[2] as usize);
    }
    // Bodies numbered by first triangle, so repeated runs split the same way.
    let mut number: Vec<Option<usize>> = vec![None; mesh.positions.len()];
    let mut bodies: Vec<Mesh> = Vec::new();
    let mut moved: Vec<u32> = vec![u32::MAX; mesh.positions.len()];
    let mut mine: Vec<usize> = vec![usize::MAX; mesh.positions.len()];
    for (i, tri) in mesh.indices.iter().enumerate() {
        if i % 4096 == 0 && give_up() {
            return None;
        }
        let root = owner.find(tri[0] as usize);
        let body = *number[root].get_or_insert_with(|| {
            bodies.push(Mesh::new());
            bodies.len() - 1
        });
        let out = &mut bodies[body];
        let mut carried = [0u32; 3];
        for (slot, &v) in carried.iter_mut().zip(tri.iter()) {
            let v = v as usize;
            // Vertices are copied into a body on first reach, tracked by side tables, in one pass.
            if mine[v] != body {
                mine[v] = body;
                moved[v] = out.positions.len() as u32;
                out.positions.push(mesh.positions[v]);
            }
            *slot = moved[v];
        }
        out.indices.push(carried);
        out.tags.push(mesh.tag(i));
    }
    Some(bodies)
}

/// Which body each vertex belongs to, as a union-find over the mesh's edges.
struct Owner {
    parent: Vec<u32>,
    rank: Vec<u8>,
}

impl Owner {
    fn of(vertices: usize) -> Owner {
        Owner { parent: (0..vertices as u32).collect(), rank: vec![0; vertices] }
    }

    fn find(&mut self, mut v: usize) -> usize {
        while self.parent[v] as usize != v {
            // Path halving: shorter lookups next time without a full compression pass.
            let grandparent = self.parent[self.parent[v] as usize];
            self.parent[v] = grandparent;
            v = grandparent as usize;
        }
        v
    }

    fn join(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        if a == b {
            return;
        }
        let (small, big) = if self.rank[a] < self.rank[b] { (a, b) } else { (b, a) };
        self.parent[small] = big as u32;
        if self.rank[small] == self.rank[big] {
            self.rank[big] += 1;
        }
    }
}
