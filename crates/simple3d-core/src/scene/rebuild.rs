//! Turning one node into another kind of node, in place.

use super::*;

impl Scene {
    /// Make a childless node an empty group, keeping its name, place, transform, colour and export mark
    /// (issue 108), so references to it stay valid when a mesh is reassembled under it. Refuses the root
    /// and nodes with children.
    pub fn make_group(&mut self, id: NodeId, op: GroupOp) -> bool {
        if id == self.root {
            return false;
        }
        let Some(node) = self.nodes.get_mut(&id) else { return false };
        if !node.children.is_empty() || node.can_hold_children() {
            return false;
        }
        node.body = Body::Group { op };
        true
    }
}
