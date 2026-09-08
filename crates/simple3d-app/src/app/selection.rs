//! What is selected, and what changing that brings with it.

use super::*;
use simple3d_core::scene::NodeId;

impl App {
    // -- selection ----------------------------------------------------------

    pub fn primary(&self) -> Option<NodeId> {
        self.selection.iter().rev().find(|id| self.scene.contains(**id)).copied()
    }

    pub fn is_selected(&self, id: NodeId) -> bool {
        self.selection.contains(&id)
    }

    pub fn select_only(&mut self, id: NodeId) {
        self.selection = vec![id];
        self.selection_anchor = Some(id);
        self.on_selection_changed();
    }

    pub fn toggle_selected(&mut self, id: NodeId) {
        if let Some(at) = self.selection.iter().position(|&x| x == id) {
            self.selection.remove(at);
        } else {
            self.selection.push(id);
        }
        // Ctrl+click puts the anchor on the row it touched, so a Shift+click
        // after it measures from where the pointer last was rather than from
        // wherever a range happened to start (issue 60).
        self.selection_anchor = Some(id);
        self.on_selection_changed();
    }

    /// Select everything between the anchor and `id`, over `rows` -- the rows
    /// the outliner is actually showing, so a range never reaches into a
    /// collapsed group the user cannot see (issue 60).
    ///
    /// Replacing the selection rather than adding to it is what Shift+click
    /// means in every list: the range is the selection, and Shift+clicking
    /// somewhere else re-measures it from the same anchor instead of piling
    /// ranges up. The anchor itself does not move, which is what lets a range
    /// be adjusted by clicking again.
    ///
    /// The scene root is left out: it is every other row's ancestor, and a
    /// selection holding it means "everything" to every command that reads one.
    pub fn select_range_to(&mut self, id: NodeId, rows: &[NodeId]) {
        let root = self.scene.root();
        let anchor = match self.selection_anchor {
            Some(anchor) if anchor != id && rows.contains(&anchor) => anchor,
            // Nothing to measure from: a Shift+click with no anchor is a plain
            // click, and sets one.
            _ => return self.select_only(id),
        };
        let (Some(from), Some(to)) =
            (rows.iter().position(|row| *row == anchor), rows.iter().position(|row| *row == id))
        else {
            return self.select_only(id);
        };
        let (lo, hi) = if from <= to { (from, to) } else { (to, from) };
        self.selection = rows[lo..=hi].iter().copied().filter(|row| *row != root).collect();
        if self.selection.is_empty() {
            self.selection = vec![id];
        }
        self.on_selection_changed();
    }

    pub fn clear_selection(&mut self) {
        self.selection.clear();
        self.on_selection_changed();
    }

    pub(super) fn on_selection_changed(&mut self) {
        // A half-typed field belongs to the node it was opened on, and so does
        // a tick in a collection's list of pieces (issue 82).
        self.fields.clear();
        self.piece_ticks.clear();
        self.history.close();
        self.rename = None;
        // Whatever is selected has to be findable: a node picked in the
        // viewport, or one left selected by an undo, opens the collapsed
        // groups above it rather than being selected out of sight.
        for id in self.selection.clone() {
            self.reveal(id);
        }
    }

    /// Open every group above `id`, so its row is one of the ones drawn.
    pub fn reveal(&mut self, id: NodeId) {
        let mut walk = self.scene.get(id).and_then(|node| node.parent);
        while let Some(parent) = walk {
            self.collapsed.remove(&parent);
            walk = self.scene.get(parent).and_then(|node| node.parent);
        }
    }

    /// Shut or open one group in the outliner.
    pub fn set_collapsed(&mut self, id: NodeId, collapsed: bool) {
        if collapsed {
            self.collapsed.insert(id);
        } else {
            self.collapsed.remove(&id);
        }
    }
}
