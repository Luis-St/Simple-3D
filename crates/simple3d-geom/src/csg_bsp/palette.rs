//! Carrying the source channel through the kernel (issue 73).
//!
//! The kernel and the repair passes carry one number per face. Rather than give each of them a second
//! one, every distinct (tag, source) pair of the operands is numbered and the number rides in the tag
//! slot; the result is translated back. Grouping by tag then also keeps faces of different sources
//! apart, as it keeps colours apart.

use crate::mesh::Mesh;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct Palette {
    pairs: Vec<(u32, u32)>,
    index: HashMap<(u32, u32), u32>,
}

impl Palette {
    /// `mesh` with each face's tag replaced by its pair's number.
    pub(super) fn encode(&mut self, mesh: &Mesh) -> Mesh {
        let tags = (0..mesh.indices.len())
            .map(|i| {
                let pair = (mesh.tag(i), mesh.source(i));
                *self.index.entry(pair).or_insert_with(|| {
                    self.pairs.push(pair);
                    (self.pairs.len() - 1) as u32
                })
            })
            .collect();
        Mesh { positions: mesh.positions.clone(), indices: mesh.indices.clone(), tags, sources: Vec::new() }
    }

    /// The numbered tags turned back into tags and sources.
    pub(super) fn decode(&self, mut mesh: Mesh) -> Mesh {
        let pair = |i: usize| self.pairs.get(mesh.tag(i) as usize).copied().unwrap_or((0, 0));
        let (tags, sources): (Vec<u32>, Vec<u32>) = (0..mesh.indices.len()).map(pair).unzip();
        mesh.tags = tags;
        // Left empty when no operand had a source, as callers without sources expect.
        mesh.sources = if sources.iter().all(|&s| s == 0) { Vec::new() } else { sources };
        mesh
    }
}
