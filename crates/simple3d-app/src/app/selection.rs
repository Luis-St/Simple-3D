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
        // Ctrl+click moves the anchor, so a following Shift+click measures from there (issue 60).
        self.selection_anchor = Some(id);
        self.on_selection_changed();
    }

    /// Select from the anchor to `id` over the visible `rows`, never into collapsed groups (issue 60).
    /// Replaces the selection, as Shift+click does in every list; the anchor stays so the range can be
    /// adjusted. The root is excluded, since selecting it means everything.
    pub fn select_range_to(&mut self, id: NodeId, rows: &[NodeId]) {
        let root = self.scene.root();
        let anchor = match self.selection_anchor {
            Some(anchor) if anchor != id && rows.contains(&anchor) => anchor,
            // No anchor: a plain click, which sets one.
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
        // A half-typed field and piece ticks belong to the previous selection (issue 82).
        self.fields.clear();
        self.piece_ticks.clear();
        self.history.close();
        self.rename = None;
        // Open collapsed groups above the selection, so it is never selected out of sight.
        for id in self.selection.clone() {
            self.reveal(id);
        }
    }

    /// Open every group above `id`, so its row is drawn.
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
