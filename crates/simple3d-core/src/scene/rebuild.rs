//! Turning one node into another kind of node, in place.

use super::*;

impl Scene {
    /// Make a node that holds no children into an empty group, keeping
    /// everything else about it -- its name, its place in the tree, its
    /// transform, its colour, its export mark (issue 108).
    ///
    /// What a mesh becomes when it is taken back apart: the objects it was
    /// found to be made of go under it, and the node standing over them is the
    /// one that was standing there before, so nothing that pointed at it -- the
    /// selection, a parent's boolean, an export body -- has to be told
    /// anything. The alternative, a fresh group put where the mesh was, would
    /// be a different node wearing its name.
    ///
    /// Refuses the root, which is a group already and the one node that may not
    /// be replaced, and refuses anything holding children: those children are
    /// the old body's operands, and a body that can hold children can be given
    /// them without this.
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
