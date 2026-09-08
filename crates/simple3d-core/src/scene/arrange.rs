//! Moving nodes about the tree: reparenting, reordering and grouping.

use super::*;
impl Scene {
    /// Move several nodes under `new_parent`, starting at `index` and keeping
    /// the order they are given in.
    ///
    /// Not a loop over `reparent` at the call site, because each single move
    /// would shift the index the next one was measured against -- and because
    /// the whole drag has to be refused as one when any part of it is illegal,
    /// rather than half-applied and then rejected. A node whose own ancestor is
    /// also being moved is left out: it travels inside it, and moving it as
    /// well would tear it out of the parent that carries it.
    pub fn reparent_many(&mut self, ids: &[NodeId], new_parent: NodeId, index: usize) -> Result<(), &'static str> {
        if !self.nodes.contains_key(&new_parent) {
            return Err("no such node");
        }
        if !self.nodes[&new_parent].can_hold_children() {
            return Err("only groups and patterns can hold children");
        }
        for &id in ids {
            if id == self.root {
                return Err("the scene root cannot be moved");
            }
            if !self.nodes.contains_key(&id) {
                return Err("no such node");
            }
            if new_parent == id || self.is_ancestor_of(id, new_parent) {
                return Err("a node cannot be moved inside itself");
            }
        }
        let mut moving: Vec<NodeId> = Vec::new();
        for &id in ids {
            let carried = ids.iter().any(|&other| other != id && self.is_ancestor_of(other, id));
            if !carried && !moving.contains(&id) {
                moving.push(id);
            }
        }
        // Index is interpreted against the target's child list *before* the
        // move, so dragging within one parent lands where the indicator showed.
        let mut index = index;
        for &id in &moving {
            if self.nodes[&new_parent].children.iter().position(|&c| c == id).is_some_and(|old| old < index) {
                index -= 1;
            }
        }
        for &id in &moving {
            self.unlink(id);
        }
        // Something dropped into a collection arrives with a row, and something
        // dragged out of one loses the mark it no longer means anything to: a
        // collection hides its *pieces*, and a node the user carried in by hand
        // is not one of them -- vanishing on release is not a move anybody aimed
        // for (issue 82).
        let into_collection = self.is_collection(new_parent);
        for (offset, &id) in moving.iter().enumerate() {
            self.link(id, new_parent, index + offset);
            if let Some(node) = self.nodes.get_mut(&id) {
                node.extracted = into_collection;
            }
        }
        Ok(())
    }

    /// Move a node up or down among its siblings. Order is semantic inside a
    /// difference group, so this has to be user-controllable.
    pub fn reorder(&mut self, id: NodeId, delta: isize) -> bool {
        let Some(parent) = self.nodes.get(&id).and_then(|n| n.parent) else { return false };
        let children = &mut self.nodes.get_mut(&parent).unwrap().children;
        let Some(from) = children.iter().position(|&c| c == id) else { return false };
        let to = from as isize + delta;
        if to < 0 || to >= children.len() as isize {
            return false;
        }
        let to = to as usize;
        children.remove(from);
        children.insert(to, id);
        true
    }

    /// Wrap the selection in a new group, preserving relative positions and
    /// order (spec section 7.2: "the single most-used structural operation").
    ///
    /// Only the topmost selected nodes are moved -- selecting a group and one of
    /// its children groups the group, not both. The new group's own position
    /// stays at the origin and children keep their coordinates, which is what
    /// keeps relative positions exactly unchanged.
    pub fn group_selection(&mut self, selection: &[NodeId]) -> Option<NodeId> {
        let mut tops: Vec<NodeId> = selection
            .iter()
            .copied()
            .filter(|&id| id != self.root && self.nodes.contains_key(&id))
            .filter(|&id| !selection.iter().any(|&other| other != id && self.is_ancestor_of(other, id)))
            .collect();
        if tops.is_empty() {
            return None;
        }
        // Group into the first selected node's parent, at its position, keeping
        // the tree's own order rather than click order.
        let parent = self.nodes[&tops[0]].parent?;
        let order = self.depth_first();
        tops.sort_by_key(|id| order.iter().position(|o| o == id).unwrap_or(usize::MAX));
        tops.retain(|&id| self.nodes[&id].parent == Some(parent));
        if tops.is_empty() {
            return None;
        }
        let index = self.nodes[&parent].children.iter().position(|c| *c == tops[0])?;
        let group = self.add_group(GroupOp::Union, parent, index);
        for (offset, id) in tops.iter().enumerate() {
            self.reparent(*id, group, offset).ok()?;
        }
        Some(group)
    }
}
