//! Operations that take a finished mesh and give back another one.
//!
//! One of them, for now: breaking a mesh into the separate pieces it is
//! actually in, which is what "break into separate objects" (issue 82) needs
//! after a cut has left a shape in more than one part.
//!
//! It works on the *welded* mesh, so a shape built by a primitive generator
//! (which pushes three fresh vertices per triangle) is read as a surface rather
//! than as a heap of separate triangles.

use crate::Mesh;
use std::collections::HashMap;

/// Break a mesh into its connected pieces (issue 82's other half): the parts a
/// cut left behind, each one a solid of its own.
///
/// Connectivity is through shared vertices of the welded mesh, which is what
/// "one piece" means for something about to be printed.
pub fn connected_parts(mesh: &Mesh) -> Vec<Mesh> {
    let welded = mesh.weld();
    if welded.indices.is_empty() {
        return Vec::new();
    }
    // Union-find over vertices, joined by every triangle.
    let mut parent: Vec<u32> = (0..welded.positions.len() as u32).collect();
    fn find(parent: &mut [u32], mut x: u32) -> u32 {
        while parent[x as usize] != x {
            parent[x as usize] = parent[parent[x as usize] as usize];
            x = parent[x as usize];
        }
        x
    }
    for tri in &welded.indices {
        let root = find(&mut parent, tri[0]);
        for &v in &tri[1..] {
            let other = find(&mut parent, v);
            if other != root {
                parent[other as usize] = root;
            }
        }
    }
    let mut groups: HashMap<u32, Mesh> = HashMap::new();
    let mut order: Vec<u32> = Vec::new();
    for (face, tri) in welded.indices.iter().enumerate() {
        let root = find(&mut parent, tri[0]);
        if !groups.contains_key(&root) {
            order.push(root);
        }
        let part = groups.entry(root).or_default();
        part.push_tagged_triangle(
            welded.positions[tri[0] as usize],
            welded.positions[tri[1] as usize],
            welded.positions[tri[2] as usize],
            welded.tag(face),
        );
    }
    // In the order the pieces are first met walking the triangles, so the
    // numbering a user sees is stable from one run to the next.
    order.into_iter().filter_map(|root| groups.remove(&root)).map(|part| part.weld()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{primitives, Vec3};

    #[test]
    fn connected_parts_finds_each_piece_once() {
        let one = primitives::box_mesh(10.0, 10.0, 10.0);
        let each = one.weld().triangle_count();
        let mut two = one.clone();
        two.append(&one.translated(Vec3::new(50.0, 0.0, 0.0)));
        let parts = connected_parts(&two);
        assert_eq!(parts.len(), 2);
        for part in &parts {
            assert!(part.manifold_issue().is_none());
            assert_eq!(part.triangle_count(), each, "a piece did not come out whole");
        }
        // And one solid is one piece, not twelve triangles.
        assert_eq!(connected_parts(&primitives::box_mesh(5.0, 5.0, 5.0)).len(), 1);
    }
}
