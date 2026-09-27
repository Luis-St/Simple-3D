//! Which subtrees are objects of their own when the model is exported.

use super::*;

impl Scene {
    /// Whether an export could consider `id`'s children one by one: not for primitives or fusing
    /// booleans; always for a split, whose pieces are separate solids.
    pub fn can_split_for_export(&self, id: NodeId) -> bool {
        self.get(id).and_then(|n| n.combine_op()).is_some_and(GroupOp::separable)
    }

    /// Every export body mark by node, in stable order, for invalidating a cached export summary.
    pub fn export_body_marks(&self) -> Vec<(NodeId, ExportBody)> {
        self.nodes.iter().filter_map(|(id, node)| node.export_body.map(|body| (*id, body))).collect()
    }

    /// The largest body number used, so a picker can offer the next; zero when nothing is grouped.
    pub fn highest_export_body(&self) -> u32 {
        self.export_body_marks()
            .into_iter()
            .filter_map(|(_, body)| match body {
                ExportBody::Shared(n) => Some(n),
                ExportBody::Split => None,
            })
            .max()
            .unwrap_or(0)
    }

    /// Give `id` a body mark, clearing marks below it that it makes unreachable, so they cannot spring
    /// back when the group is split again.
    pub fn set_export_body(&mut self, id: NodeId, body: Option<ExportBody>) {
        if let Some(node) = self.get_mut(id) {
            node.export_body = body;
        }
        if body != Some(ExportBody::Split) {
            for descendant in self.descendants(id) {
                if let Some(node) = self.get_mut(descendant) {
                    node.export_body = None;
                }
            }
        }
    }
}
