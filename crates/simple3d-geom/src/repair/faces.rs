//! Cancelling face pairs that occupy the same place facing opposite ways.

use crate::mesh::Mesh;
use std::collections::HashMap;

/// Drop triangle pairs that describe the same three vertices with opposite
/// winding. They are two coincident, oppositely-facing surface patches that
/// enclose no volume, which a boolean can legitimately produce when an
/// operand's face lies exactly on the result's boundary; leaving them in makes
/// every one of their edges used twice in the same direction.
pub(crate) fn cancel_opposite_faces(mesh: Mesh) -> Mesh {
    let key = |t: &[u32; 3]| {
        let mut k = *t;
        k.sort_unstable();
        k
    };
    let mut by_key: HashMap<[u32; 3], Vec<usize>> = HashMap::new();
    for (i, t) in mesh.indices.iter().enumerate() {
        by_key.entry(key(t)).or_default().push(i);
    }
    let mut dead = vec![false; mesh.indices.len()];
    for group in by_key.values() {
        if group.len() < 2 {
            continue;
        }
        // Winding sign relative to the first triangle of the group.
        let mut pos: Vec<usize> = Vec::new();
        let mut neg: Vec<usize> = Vec::new();
        let reference = mesh.indices[group[0]];
        for &i in group {
            if same_winding(&reference, &mesh.indices[i]) {
                pos.push(i);
            } else {
                neg.push(i);
            }
        }
        for _ in 0..pos.len().min(neg.len()) {
            dead[pos.pop().unwrap()] = true;
            dead[neg.pop().unwrap()] = true;
        }
    }
    let (indices, tags) =
        mesh.indices.iter().enumerate().filter(|(i, _)| !dead[*i]).map(|(i, t)| (*t, mesh.tag(i))).unzip();
    Mesh { positions: mesh.positions, indices, tags }
}

pub(crate) fn same_winding(a: &[u32; 3], b: &[u32; 3]) -> bool {
    for r in 0..3 {
        if b[0] == a[r] && b[1] == a[(r + 1) % 3] && b[2] == a[(r + 2) % 3] {
            return true;
        }
    }
    false
}
