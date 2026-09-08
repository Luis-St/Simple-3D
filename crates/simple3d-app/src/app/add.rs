//! Adding a node, and where in the tree it lands.

use super::*;
use simple3d_core::scene::{GroupOp, NodeId};

impl App {
    /// Add an empty pattern, for shapes to be put under it afterwards
    /// (issue 67).
    ///
    /// The "add a container" gesture, which is why it sits beside "Union group"
    /// in both Add menus. `Command::Pattern` is the other half of the same
    /// feature and does the opposite thing -- it wraps whatever is selected --
    /// so this one always makes an empty pattern, whatever is selected.
    pub fn add_pattern_at(&mut self, at: NodeId) {
        let where_to = self.insertion_from_row(at);
        self.add_empty_pattern(where_to);
    }

    /// The same, at the document's own insertion point rather than at a row.
    pub fn add_pattern(&mut self) {
        let where_to = self.scene.insertion_point(self.primary());
        self.add_empty_pattern(where_to);
    }

    pub(super) fn add_empty_pattern(&mut self, (parent, index): (NodeId, usize)) {
        self.edit("Add pattern", None);
        let id = self.scene.add_pattern(parent, index);
        self.collapsed.remove(&parent);
        self.select_only(id);
        self.status = Status::Info("Added an empty pattern: put shapes into it and it repeats them".into());
    }

    /// Add a shape dragged out of the palette, exactly where the drop indicator
    /// said it would land.
    ///
    /// Where a shape *added* lands is a document setting -- at the view centre,
    /// beside the selection, at the 3D cursor -- and this keeps to it: the drag
    /// says which row of the tree the shape belongs on, not where in the world
    /// it sits, and the two are separate questions.
    pub fn add_dropped_primitive(&mut self, type_id: &str, parent: NodeId, index: usize) {
        self.edit("Add", None);
        let Some(id) = self.scene.add_primitive(type_id, parent, index) else {
            self.history.discard_last();
            self.status = Status::Warning("That shape is not in the palette".into());
            return;
        };
        let at = self.insertion_point_world(self.near_face_x(&[id]).unwrap_or(0.0));
        if let Some(node) = self.scene.get_mut(id) {
            node.position = at;
        }
        self.collapsed.remove(&parent);
        self.select_only(id);
        self.status = Status::Info(format!("Added {}", self.scene.node(id).name));
    }

    pub fn add_node_at(&mut self, at: NodeId, type_id: Option<&str>, op: GroupOp) {
        self.edit(if type_id.is_some() { "Add" } else { "Add group" }, None);
        let (parent, index) = self.insertion_from_row(at);
        let created = match type_id {
            Some(type_id) => self.scene.add_primitive(type_id, parent, index),
            None => Some(self.scene.add_group(op, parent, index)),
        };
        let Some(id) = created else {
            self.history.discard_last();
            self.status = Status::Warning("That shape is not in the palette".into());
            return;
        };
        // A pattern that has just gained its first child can now be measured.
        self.size_fresh_patterns();
        self.collapsed.remove(&parent);
        self.select_only(id);
        self.status = Status::Info(format!("Added {}", self.scene.node(id).name));
    }
}
