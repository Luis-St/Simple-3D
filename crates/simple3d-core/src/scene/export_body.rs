//! Which subtrees are objects of their own when the model is exported.

use super::*;

impl Scene {
    /// What a node is painted, following the tree upward: its own colour, else
    /// the nearest painted ancestor's, else nothing at all. This is what makes
    /// painting a group paint every shape inside it without touching any of
    /// them, and what a shape painted inside a painted group overrides.
    /// Whether `id` is a node whose children an export could consider one by
    /// one. A primitive has no parts, and a boolean that fuses its operands has
    /// none that survive it. A split's children are separate solids by
    /// construction -- that is what breaking a shape apart found -- so it always
    /// can.
    pub fn can_split_for_export(&self, id: NodeId) -> bool {
        self.get(id).and_then(|n| n.combine_op()).is_some_and(GroupOp::separable)
    }

    /// Every export body mark in the scene, by node, in a stable order. Small
    /// -- a scene nobody has grouped has none -- and it is what tells a cached
    /// export summary that the grouping has been edited under it.
    pub fn export_body_marks(&self) -> Vec<(NodeId, ExportBody)> {
        self.nodes.iter().filter_map(|(id, node)| node.export_body.map(|body| (*id, body))).collect()
    }

    /// The largest body number used anywhere, so a picker can offer the next
    /// one. Zero when nothing is grouped, which makes the first offer "Body 1".
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

    /// Give `id` a body mark, clearing anything below it that the mark makes
    /// unreachable: marking a group as a body of its own leaves the marks
    /// inside it saying something that is no longer true, and a stale mark that
    /// springs back when the group is split again is worse than none.
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
