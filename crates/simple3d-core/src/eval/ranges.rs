//! Which vertices of a result each node below it owns.

use super::*;
use crate::scene::{NodeId, Scene};
use std::collections::BTreeMap;
use std::ops::Range;

impl Evaluator {
    /// Which vertices of `id`'s mesh each node below it owns ([`Evaluated::ranges`]), with `id`
    /// starting at `start`. Walked from the tree, since cached results are shared across nodes.
    pub(super) fn ranges(
        &mut self,
        scene: &Scene,
        id: NodeId,
        start: u32,
        cancel: &Cancel,
        out: &mut BTreeMap<NodeId, Range<u32>>,
    ) {
        let result = self.subtree(scene, id, cancel);
        out.insert(id, start..start + result.mesh.positions.len() as u32);
        let visible: Vec<NodeId> =
            scene.node(id).children.iter().copied().filter(|&child| scene.node(child).visible).collect();
        for &(place, offset) in &result.passed {
            if let Some(&child) = visible.get(place) {
                self.ranges(scene, child, start + offset, cancel, out);
            }
        }
    }
}
