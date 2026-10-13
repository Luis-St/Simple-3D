//! Visibility, sibling order, and a node's group operation.

use super::*;
use simple3d_core::scene::{GroupOp, NodeId};

impl App {
    pub(super) fn toggle_visibility(&mut self) {
        let targets = self.top_level_selection();
        if targets.is_empty() {
            return;
        }
        self.edit("Toggle visibility", None);
        // Everything follows the primary node, so a mixed selection ends up consistent.
        let target_state = self.primary().map(|id| !self.scene.node(id).visible).unwrap_or(false);
        for id in targets {
            if let Some(node) = self.scene.get_mut(id) {
                node.visible = target_state;
            }
        }
    }

    /// Whether the selection holds anything that can be a negative part: anything but the root.
    pub fn can_toggle_negative(&self) -> bool {
        self.top_level_selection().into_iter().any(|id| id != self.scene.root())
    }

    /// Mark the selection as negative parts, cut from their siblings (issue 119), or back to solid.
    /// Like visibility, everything follows the primary node.
    pub(crate) fn toggle_negative(&mut self) {
        let root = self.scene.root();
        let targets: Vec<NodeId> = self.top_level_selection().into_iter().filter(|&id| id != root).collect();
        let Some(&first) = targets.last() else { return };
        let primary = self.primary().filter(|id| targets.contains(id)).unwrap_or(first);
        self.set_negative(&targets, !self.scene.node(primary).negative);
    }

    /// Make `targets` negative parts or solid, as one undo step; the root is never one.
    pub(crate) fn set_negative(&mut self, targets: &[NodeId], negative: bool) {
        let root = self.scene.root();
        let changes =
            targets.iter().any(|&id| id != root && self.scene.get(id).is_some_and(|n| n.negative != negative));
        if !changes {
            return;
        }
        self.edit(if negative { "Make negative part" } else { "Make solid" }, None);
        for &id in targets.iter().filter(|&&id| id != root) {
            if let Some(node) = self.scene.get_mut(id) {
                node.negative = negative;
            }
        }
    }

    /// Whether a move by `delta` would move anything, for greying out menu entries.
    pub fn can_reorder(&self, delta: isize) -> bool {
        !self.reorder_plan(delta).is_empty()
    }

    /// What a reorder acts on: the topmost selected nodes in tree order, minus the root, so all
    /// selected siblings move (issue 41).
    pub(super) fn reorder_targets(&self) -> Vec<NodeId> {
        self.top_level_selection().into_iter().filter(|id| *id != self.scene.root()).collect()
    }

    /// Which of those would move, in move order. Selected siblings move as a block, so a node blocked
    /// by an unmovable selected neighbour stays too.
    pub(super) fn reorder_plan(&self, delta: isize) -> Vec<NodeId> {
        let mut targets = self.reorder_targets();
        if delta > 0 {
            targets.reverse();
        }
        let mut stuck: Vec<NodeId> = Vec::new();
        let mut moving: Vec<NodeId> = Vec::new();
        for id in targets {
            let Some(parent) = self.scene.get(id).and_then(|node| node.parent) else {
                stuck.push(id);
                continue;
            };
            let children = &self.scene.node(parent).children;
            let Some(at) = children.iter().position(|&c| c == id) else {
                stuck.push(id);
                continue;
            };
            let to = at as isize + delta;
            if to < 0 || to >= children.len() as isize {
                stuck.push(id);
                continue;
            }
            if stuck.contains(&children[to as usize]) {
                stuck.push(id);
                continue;
            }
            moving.push(id);
        }
        moving
    }

    pub(super) fn reorder(&mut self, delta: isize) {
        if self.reorder_targets().is_empty() {
            self.status = Status::Info("Select something to move".into());
            return;
        }
        let plan = self.reorder_plan(delta);
        if plan.is_empty() {
            self.status = Status::Info(
                if delta < 0 { "Already first among its siblings" } else { "Already last among its siblings" }.into(),
            );
            return;
        }
        self.edit("Reorder", None);
        for id in plan {
            self.scene.reorder(id, delta);
        }
    }

    /// Set a group's boolean operator.
    pub fn set_group_op(&mut self, id: NodeId, op: GroupOp) {
        if self.scene.node(id).group_op() == Some(op) {
            return;
        }
        self.edit("Operation", None);
        if let Some(node) = self.scene.get_mut(id) {
            node.body = simple3d_core::scene::Body::Group { op };
        }
        self.status = Status::Info(format!("{} group", op.label()));
    }

    /// Where a node added from an outliner row lands (issue 44): inside it if it can hold children
    /// (patterns too, issue 67), beside it otherwise.
    pub(crate) fn insertion_from_row(&self, at: NodeId) -> (NodeId, usize) {
        let end_of_root = (self.scene.root(), self.scene.node(self.scene.root()).children.len());
        match self.scene.get(at) {
            // Not into a collection, which the tree does not open (issue 82).
            Some(node) if node.can_hold_children() && !node.is_split() => (at, self.scene.node(at).children.len()),
            Some(node) => match node.parent {
                Some(parent) => {
                    let after = self.scene.node(parent).children.iter().position(|&c| c == at).map_or(0, |i| i + 1);
                    (parent, after)
                }
                None => end_of_root,
            },
            None => end_of_root,
        }
    }
}
