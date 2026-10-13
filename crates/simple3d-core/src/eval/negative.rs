//! Negative parts (issue 119): shown children cut from their siblings' result in any group.

use super::*;
use crate::scene::{GroupOp, NodeId, Scene};
use simple3d_geom::Mesh;

impl Evaluator {
    /// The meshes of `id`'s shown negative children, with their errors added to `errors`.
    pub(super) fn negatives(
        &mut self,
        scene: &Scene,
        id: NodeId,
        errors: &mut Vec<NodeError>,
        cancel: &Cancel,
    ) -> Vec<Mesh> {
        let mut out = Vec::new();
        for &child in &scene.node(id).children {
            if !scene.node(child).cuts() {
                continue;
            }
            let result = self.subtree(scene, child, cancel);
            errors.extend(result.errors.iter().cloned());
            if result.mesh.triangle_count() > 0 {
                out.push((*result.mesh).clone());
            }
        }
        out
    }
}

/// `mesh` with `negatives` cut out of it. The children in `passed` stay where they were only when
/// nothing was cut, since the cut rebuilds the vertices.
pub(super) fn cut_negatives(
    mesh: Mesh,
    negatives: Vec<Mesh>,
    passed: &mut Vec<(usize, u32)>,
    id: NodeId,
    name: &str,
    errors: &mut Vec<NodeError>,
    cancel: &Cancel,
) -> Mesh {
    if negatives.is_empty() || mesh.triangle_count() == 0 {
        return mesh;
    }
    let mut operands = Vec::with_capacity(negatives.len() + 1);
    operands.push(mesh);
    operands.extend(negatives);
    let (result, untouched) = combine(GroupOp::Difference, &operands, id, name, errors, cancel);
    match untouched.first() {
        Some(&(0, offset)) => passed.iter_mut().for_each(|(_, start)| *start += offset),
        _ => passed.clear(),
    }
    result
}
