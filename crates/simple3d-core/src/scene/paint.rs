//! The colour a node ends up with, and painting a subtree.

use super::*;
impl Scene {
    pub fn effective_colour(&self, id: NodeId) -> Option<Colour> {
        let mut at = Some(id);
        while let Some(node) = at.and_then(|id| self.nodes.get(&id)) {
            if node.colour.is_some() {
                return node.colour;
            }
            at = node.parent;
        }
        None
    }

    /// Whether this node, or anything under it, carries a colour of its own --
    /// which is exactly when clearing has anything to do. A node that merely
    /// inherits its colour from a group above it has nothing of its own to
    /// clear, and offering to clear it would be a control that does nothing.
    pub fn subtree_is_painted(&self, id: NodeId) -> bool {
        let mut stack = vec![id];
        while let Some(at) = stack.pop() {
            let Some(node) = self.nodes.get(&at) else { continue };
            if node.colour.is_some() {
                return true;
            }
            stack.extend(node.children.iter().copied());
        }
        false
    }

    /// Paint a node and everything under it. Clearing the descendants' own
    /// colours is the point: "paint this group red" means the whole group turns
    /// red, not that the shapes which were painted individually keep their own.
    /// Passing `None` strips the colour from the subtree entirely.
    pub fn paint_subtree(&mut self, id: NodeId, colour: Option<Colour>) {
        let mut stack = vec![id];
        while let Some(at) = stack.pop() {
            let Some(node) = self.nodes.get_mut(&at) else { continue };
            node.colour = if at == id { colour } else { None };
            stack.extend(node.children.iter().copied());
        }
    }

    /// The effective segment count for a node: its own override, else the
    /// scene default.
    pub fn segments_for(&self, id: NodeId) -> u32 {
        self.nodes.get(&id).and_then(|n| n.segments).unwrap_or(self.settings.default_segments).clamp(3, 512)
    }
}
