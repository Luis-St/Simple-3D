//! Visibility, sibling order, and the group operation a node carries.

use super::*;
use simple3d_core::scene::{GroupOp, NodeId};

impl App {
    pub(super) fn toggle_visibility(&mut self) {
        let targets = self.top_level_selection();
        if targets.is_empty() {
            return;
        }
        self.edit("Toggle visibility", None);
        // Everything follows the primary node, so a mixed selection ends up
        // consistent rather than inverted node by node.
        let target_state = self.primary().map(|id| !self.scene.node(id).visible).unwrap_or(false);
        for id in targets {
            if let Some(node) = self.scene.get_mut(id) {
                node.visible = target_state;
            }
        }
    }

    /// Whether a move by `delta` would do anything: at least one of the nodes
    /// it acts on has somewhere to go. What the menus grey their entries out
    /// on, rather than letting a click answer with a status line nobody sees.
    pub fn can_reorder(&self, delta: isize) -> bool {
        !self.reorder_plan(delta).is_empty()
    }

    /// What a reorder acts on: the topmost selected nodes, in tree order,
    /// minus the root. The whole selection rather than the primary alone --
    /// moving one of three selected siblings and leaving the other two behind
    /// looks exactly like the command not working (issue 41).
    pub(super) fn reorder_targets(&self) -> Vec<NodeId> {
        self.top_level_selection().into_iter().filter(|id| *id != self.scene.root()).collect()
    }

    /// Which of those nodes would actually move, in the order they have to be
    /// moved in.
    ///
    /// Selected siblings move as a block: the one nearest the end goes first,
    /// into the space it has, and a node whose neighbour is a selected node
    /// that could not move cannot move either -- otherwise a run of three
    /// pushed against the end of the list would come apart, one node
    /// overtaking another that had nowhere to go.
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

    /// Set a group's boolean operator, from wherever a group can be pointed at.
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

    /// Add a group or a primitive *where the tree is pointing*: inside `at` when
    /// it can hold children, beside it otherwise. What the outliner's own Add
    /// menu uses, so a shape made from a row lands on that row rather than at the
    /// document's insertion point (issue 44).
    /// Where a node added *from an outliner row* lands: inside the row when it
    /// can hold children, beside it otherwise (issue 44).
    ///
    /// A pattern holds children exactly as a group does (issue 67), so Add from
    /// a pattern's own row goes *into* it. Asking `is_group` here put the shape
    /// beside the pattern instead, which made the row menu disagree with both
    /// the drag-and-drop rule and the document-level Add, and both of those
    /// already say "into".
    pub(super) fn insertion_from_row(&self, at: NodeId) -> (NodeId, usize) {
        let end_of_root = (self.scene.root(), self.scene.node(self.scene.root()).children.len());
        match self.scene.get(at) {
            // Not into a collection: the tree does not open one, so a shape
            // added on its row would land somewhere it cannot be seen
            // (issue 82).
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
