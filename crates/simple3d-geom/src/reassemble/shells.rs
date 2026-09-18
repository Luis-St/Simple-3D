//! The separate bodies one mesh holds.

use super::*;
use crate::mesh::Mesh;

/// Break a welded mesh into the connected pieces it is in, each with its own
/// vertices and its tags carried across.
///
/// Connected through *shared vertices*, which is what a weld leaves and what
/// separateness means here: two solids modelled side by side and flattened into
/// one mesh have no vertex in common however close they stand, and two halves
/// of one surface share every vertex along the seam between them. Touching is
/// therefore not connection -- a pin resting in a hole is two bodies -- which is
/// the right answer, and is why bodies that touch are offered a group rather
/// than merged (see [`super::group`]).
///
/// The caller welds first; a mesh whose triangles each carry their own copies
/// of their corners is one body per triangle.
pub(super) fn shells(mesh: &Mesh, give_up: Abandon<'_>) -> Option<Vec<Mesh>> {
    if mesh.indices.is_empty() {
        return Some(Vec::new());
    }
    let mut owner = Owner::of(mesh.positions.len());
    for (i, tri) in mesh.indices.iter().enumerate() {
        // Often enough that a mesh of a hundred thousand triangles can be
        // abandoned promptly, and rarely enough that the check is not the work.
        if i % 4096 == 0 && give_up() {
            return None;
        }
        owner.join(tri[0] as usize, tri[1] as usize);
        owner.join(tri[1] as usize, tri[2] as usize);
    }
    // One pass to number the bodies in the order their first triangle appears,
    // so a mesh taken apart twice comes apart the same way.
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
            // A vertex is copied into a body the first time that body reaches
            // it, and the two side tables say where it went -- so the copy is
            // one pass over the triangles rather than a map per body.
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
            // Halving: the next look-up of anything on this path is one step
            // shorter, without the second pass a full compression costs.
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
