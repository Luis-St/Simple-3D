//! Reaching nodes: by id, by walk, and by position in the tree.

use super::*;
use std::collections::{BTreeMap, HashSet};

impl Scene {
    pub fn new() -> Scene {
        let root = Node::fresh(1, "Scene".to_string(), Body::Group { op: GroupOp::Union }, None);
        let mut nodes = BTreeMap::new();
        nodes.insert(1, root);
        Scene {
            nodes,
            root: 1,
            next_id: 2,
            settings: SceneSettings::default(),
            camera: Camera::default(),
            components: Default::default(),
        }
    }

    pub fn root(&self) -> NodeId {
        self.root
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes[&self.root].children.is_empty()
    }

    pub fn contains(&self, id: NodeId) -> bool {
        self.nodes.contains_key(&id)
    }

    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(&id)
    }

    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(&id)
    }

    /// The node, or a panic, for callers that know the id is live. `#[track_caller]` so the panic
    /// names the caller that kept a stale id.
    #[track_caller]
    pub fn node(&self, id: NodeId) -> &Node {
        match self.nodes.get(&id) {
            Some(node) => node,
            None => panic!("node {id} is not in the scene"),
        }
    }

    pub fn ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.nodes.keys().copied()
    }

    pub(super) fn fresh_id(&mut self) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Depth-first order, root first: the outliner's order.
    pub fn depth_first(&self) -> Vec<NodeId> {
        let mut out = Vec::with_capacity(self.nodes.len());
        self.push_depth_first(self.root, &mut out);
        out
    }

    pub(super) fn push_depth_first(&self, id: NodeId, out: &mut Vec<NodeId>) {
        out.push(id);
        for &child in &self.nodes[&id].children {
            self.push_depth_first(child, out);
        }
    }

    pub fn descendants(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        self.push_depth_first(id, &mut out);
        out.remove(0);
        out
    }

    pub fn is_ancestor_of(&self, ancestor: NodeId, mut node: NodeId) -> bool {
        while let Some(parent) = self.nodes.get(&node).and_then(|n| n.parent) {
            if parent == ancestor {
                return true;
            }
            node = parent;
        }
        false
    }

    /// Whether a node is actually drawn: neither it nor any ancestor hidden.
    pub fn is_shown(&self, id: NodeId) -> bool {
        let mut at = Some(id);
        while let Some(node) = at.and_then(|id| self.nodes.get(&id)) {
            if !node.visible {
                return false;
            }
            at = node.parent;
        }
        true
    }

    pub fn depth(&self, mut id: NodeId) -> usize {
        let mut d = 0;
        while let Some(parent) = self.nodes.get(&id).and_then(|n| n.parent) {
            d += 1;
            id = parent;
        }
        d
    }

    /// Where a new node goes given the selection: into a group, else right after it (spec sections 7.2, 8.1).
    pub fn insertion_point(&self, selection: Option<NodeId>) -> (NodeId, usize) {
        match selection.and_then(|id| self.nodes.get(&id)) {
            // Not into a collection, which the tree does not open (issue 82); dragging in is still allowed.
            Some(node) if node.can_hold_children() && !node.is_split() => (node.id, node.children.len()),
            Some(node) => {
                let parent = node.parent.unwrap_or(self.root);
                let index = self.nodes[&parent].children.iter().position(|&c| c == node.id).map_or(0, |i| i + 1);
                (parent, index)
            }
            None => (self.root, self.nodes[&self.root].children.len()),
        }
    }

    /// Every name in the document, which a new name must avoid.
    pub fn taken_names(&self) -> HashSet<String> {
        self.nodes.values().map(|n| n.name.clone()).collect()
    }

    /// A name unique across the whole document, since the outliner shows every depth at once.
    pub(super) fn unique_name(&self, base: &str) -> String {
        free_name(&self.taken_names(), base)
    }
}
