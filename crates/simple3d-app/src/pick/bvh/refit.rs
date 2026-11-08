//! Keeping a tree right while its mesh's vertices move (the brush, issue 127): the boxes of the
//! leaves holding moved triangles are measured again and their ancestors grown or shrunk to fit,
//! bottom up. The triangles stay in the leaves they were sorted into, so a tree refit after a long
//! stroke is looser than a rebuilt one, but it answers the same.

use super::*;
use std::collections::BinaryHeap;

/// Each node's parent and each triangle's leaf, found once per tree.
pub(crate) struct Refit {
    parent: Vec<u32>,
    leaf: Vec<u32>,
}

impl Bvh {
    /// What refitting this tree needs; built once, since the tree's shape never changes.
    pub(crate) fn refit_map(&self) -> Refit {
        let mut parent = vec![0u32; self.nodes.len()];
        let mut leaf = vec![0u32; self.order.len()];
        for (at, node) in self.nodes.iter().enumerate() {
            if node.count == 0 {
                parent[node.first as usize] = at as u32;
                parent[node.first as usize + 1] = at as u32;
                continue;
            }
            let first = node.first as usize;
            for &triangle in &self.order[first..first + node.count as usize] {
                leaf[triangle as usize] = at as u32;
            }
        }
        Refit { parent, leaf }
    }

    /// Fit the boxes again around `triangles`, which moved in `mesh`.
    ///
    /// A child always comes after its parent in `nodes` (`grow` appends children, and subtrees
    /// built apart are appended after their reserved root), so taking the highest dirty node first
    /// finishes every node's children before it.
    pub(crate) fn refit(&mut self, map: &Refit, mesh: &Mesh, triangles: &[u32]) {
        let mut dirty: BinaryHeap<u32> = triangles.iter().map(|&t| map.leaf[t as usize]).collect();
        let mut last = u32::MAX;
        while let Some(at) = dirty.pop() {
            if at == last {
                continue;
            }
            last = at;
            let node = self.nodes[at as usize];
            let (lo, hi) = if node.count > 0 {
                let first = node.first as usize;
                let (lo, hi) = self.order[first..first + node.count as usize].iter().fold(empty(), |(lo, hi), &t| {
                    let [a, b, c] = mesh.indices[t as usize].map(|corner| mesh.positions[corner as usize]);
                    (lo.min(a).min(b).min(c), hi.max(a).max(b).max(c))
                });
                padded(lo, hi)
            } else {
                let (left, right) = (self.nodes[node.first as usize], self.nodes[node.first as usize + 1]);
                (left.lo.min(right.lo), left.hi.max(right.hi))
            };
            self.nodes[at as usize].lo = lo;
            self.nodes[at as usize].hi = hi;
            if at != 0 {
                dirty.push(map.parent[at as usize]);
            }
        }
    }
}
