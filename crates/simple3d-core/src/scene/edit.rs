//! Linking, unlinking, removing and duplicating.

use super::*;
use std::collections::HashSet;

impl Scene {
    pub(super) fn link(&mut self, id: NodeId, parent: NodeId, index: usize) {
        let children = &mut self.nodes.get_mut(&parent).expect("parent exists").children;
        let index = index.min(children.len());
        children.insert(index, id);
        self.nodes.get_mut(&id).unwrap().parent = Some(parent);
    }

    pub(super) fn unlink(&mut self, id: NodeId) {
        if let Some(parent) = self.nodes[&id].parent {
            if let Some(p) = self.nodes.get_mut(&parent) {
                p.children.retain(|&c| c != id);
            }
        }
    }

    /// Delete a subtree. The root is protected (spec section 7.2).
    pub fn remove(&mut self, id: NodeId) -> bool {
        if id == self.root || !self.nodes.contains_key(&id) {
            return false;
        }
        self.unlink(id);
        for descendant in self.descendants(id) {
            self.nodes.remove(&descendant);
        }
        self.nodes.remove(&id);
        true
    }

    /// Deep copy with fresh identities, inserted directly after the original
    /// (spec section 7.2). Every property is preserved.
    pub fn duplicate(&mut self, id: NodeId) -> Option<NodeId> {
        if id == self.root {
            return None;
        }
        let parent = self.nodes.get(&id)?.parent?;
        let index = self.nodes[&parent].children.iter().position(|&c| c == id)? + 1;
        let mut data = self.export_subtree(id)?;
        // A duplicate is a copy of something already in the document, so it is
        // named the way a pasted copy is -- "Box copy", not a second "Box".
        data.name = free_name(&self.taken_names(), &copy_name(&data.name));
        let new_id = self.import_subtree(&data, parent, index)?;
        self.rename_subtree_uniquely(new_id, true);
        Some(new_id)
    }

    /// Give every node of a freshly imported subtree a name no other node in the
    /// document carries.
    ///
    /// `keep_top` is for a top whose name was already chosen against the
    /// document -- a duplicate's "Box copy", a paste's -- so it is not put
    /// through the rule twice and does not come out "Box copy 2" when nothing
    /// clashed.
    pub fn rename_subtree_uniquely(&mut self, id: NodeId, keep_top: bool) {
        let subtree: HashSet<NodeId> = std::iter::once(id).chain(self.descendants(id)).collect();
        let mut taken: HashSet<String> =
            self.nodes.values().filter(|n| !subtree.contains(&n.id)).map(|n| n.name.clone()).collect();
        if keep_top {
            taken.insert(self.nodes[&id].name.clone());
        }
        let mut stack = if keep_top { self.nodes[&id].children.clone() } else { vec![id] };
        stack.reverse();
        while let Some(node) = stack.pop() {
            let name = free_name(&taken, &self.nodes[&node].name);
            taken.insert(name.clone());
            self.nodes.get_mut(&node).unwrap().name = name;
            for &child in self.nodes[&node].children.iter().rev() {
                stack.push(child);
            }
        }
    }

    /// Move `id` under `new_parent` at `index`. Refuses to create a cycle and
    /// refuses to move the root (spec section 7.2).
    pub fn reparent(&mut self, id: NodeId, new_parent: NodeId, index: usize) -> Result<(), &'static str> {
        self.reparent_many(&[id], new_parent, index)
    }
}
