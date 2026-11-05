//! A mesh's vertices grouped into the bodies they are connected through.

use super::*;

/// Each vertex's body: vertices joined by triangles share one, found by union-find. Bodies are
/// numbered from 0 in order of their lowest vertex, so the numbering only depends on the mesh.
pub fn connected_bodies(mesh: &Mesh) -> Vec<u32> {
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
    // A root's number, once given; a vector rather than a map, since roots are vertices.
    let mut numbers = vec![u32::MAX; parent.len()];
    let mut next = 0;
    (0..parent.len() as u32)
        .map(|vertex| {
            let root = find(&mut parent, vertex) as usize;
            if numbers[root] == u32::MAX {
                numbers[root] = next;
                next += 1;
            }
            numbers[root]
        })
        .collect()
}
