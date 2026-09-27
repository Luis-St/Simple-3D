//! Moving nodes about the tree: reparenting, reordering and grouping.

use super::*;
impl Scene {
    /// Move several nodes under `new_parent` from `index`, in the given order. One call, since single
    /// moves would shift each other's indices and an illegal drag must be refused whole. Nodes whose
    /// ancestor also moves are skipped, travelling inside it.
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
        // The index refers to the target's children before the move, matching the indicator.
        let mut index = index;
        for &id in &moving {
            if self.nodes[&new_parent].children.iter().position(|&c| c == id).is_some_and(|old| old < index) {
                index -= 1;
            }
        }
        for &id in &moving {
            self.unlink(id);
        }
        // Dropped into a collection gives a row; dragged out drops the mark, since a hand-carried node is
        // not a hidden piece (issue 82).
        let into_collection = self.is_collection(new_parent);
        for (offset, &id) in moving.iter().enumerate() {
            self.link(id, new_parent, index + offset);
            if let Some(node) = self.nodes.get_mut(&id) {
                node.extracted = into_collection;
            }
        }
        Ok(())
    }

    /// Move a node up or down among its siblings; order matters in a difference.
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

    /// Wrap the selection in a new group, keeping relative positions and order (spec section 7.2).
    /// Only topmost nodes move; the group stays at the origin so children keep their coordinates.
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
        // Into the first selected node's parent at its position, in tree order.
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
