//! Taking pieces out of a collection and putting them back.

use super::*;
use simple3d_core::scene::NodeId;

impl App {
    /// Give the ticked pieces their own rows (issue 82). Extracting every piece goes through
    /// [`App::extract_all_pieces`], which drops the recipe.
    pub fn extract_ticked_pieces(&mut self, collection: NodeId) {
        let ticked: Vec<NodeId> = self.piece_ticks.iter().copied().collect();
        if ticked.is_empty() {
            self.status = Status::Warning("Tick the pieces to extract first".into());
            return;
        }
        let inside: Vec<NodeId> =
            self.scene.node(collection).children.iter().copied().filter(|c| !self.scene.node(*c).extracted).collect();
        if inside.iter().all(|id| ticked.contains(id)) {
            // The last piece out empties the collection, which must ask first.
            self.ask_to_extract_all(collection);
            return;
        }
        self.edit("Extract pieces", None);
        let moved = self.scene.extract_pieces(collection, &ticked);
        if moved == 0 {
            self.history.discard_last();
            self.status = Status::Info("Those pieces are already extracted".into());
            return;
        }
        self.collapsed.remove(&collection);
        // The ticks stay, so Put back works at once.
        self.status = Status::Info(format!("Extracted {} {}", moved, if moved == 1 { "piece" } else { "pieces" }));
    }

    /// Put the ticked pieces back inside the collection, losing their rows.
    pub fn return_ticked_pieces(&mut self, collection: NodeId) {
        let ticked: Vec<NodeId> = self.piece_ticks.iter().copied().collect();
        if ticked.is_empty() {
            self.status = Status::Warning("Tick the pieces to put back first".into());
            return;
        }
        self.edit("Put pieces back", None);
        let moved = self.scene.return_pieces(collection, &ticked);
        if moved == 0 {
            self.history.discard_last();
            self.status = Status::Info("Those pieces are already inside".into());
            return;
        }
        // Still ticked, so they can go straight back out and the viewport shows which moved.
        self.status = Status::Info(format!("Put {} {} back", moved, if moved == 1 { "piece" } else { "pieces" }));
    }

    /// Ask before emptying a collection, which turns it into a union group and drops its original (issue 82).
    pub fn ask_to_extract_all(&mut self, collection: NodeId) {
        if !self.scene.is_collection(collection) {
            return;
        }
        self.confirm_extract = Some(collection);
        self.modal = Modal::ConfirmExtractAll;
    }

    /// Extract every piece, turning the collection into an ordinary union group (issue 82).
    pub fn extract_all_pieces(&mut self, collection: NodeId) {
        if !self.scene.is_collection(collection) {
            return;
        }
        let count = self.scene.node(collection).children.len();
        let name = self.scene.node(collection).name.clone();
        self.edit("Extract every piece", None);
        self.scene.dissolve_collection(collection);
        self.collapsed.remove(&collection);
        self.piece_ticks.clear();
        self.select_only(collection);
        self.status = Status::Info(format!(
            "Extracted {count} {} -- {name} is a union group now",
            if count == 1 { "piece" } else { "pieces" }
        ));
    }

    /// How to undo a split, for the status line, so the split is seen to be reversible.
    pub(crate) fn way_back(&self) -> String {
        let shortcut = self.keymap.shortcut_text(simple3d_core::keymap::Command::Rejoin);
        if shortcut.is_empty() {
            "they can be joined back together".to_string()
        } else {
            format!("{shortcut} joins them back together")
        }
    }

    /// Put a split shape back together (issue 82): the original returns and the pieces go. The split's
    /// transform is kept; edits to individual pieces are not. An ordinary undoable step.
    pub fn rejoin_selection(&mut self) {
        let targets = self.top_level_selection();
        let Some(&id) = targets.first() else {
            self.status = Status::Warning("Select a shape that was split into pieces to join back together".into());
            return;
        };
        if targets.len() > 1 {
            self.status = Status::Warning("Join one split shape back together at a time".into());
            return;
        }
        if !self.scene.node(id).is_split() {
            self.status =
                Status::Warning("Only something that was split into pieces can be joined back together".into());
            return;
        }
        let pieces = self.scene.node(id).children.len();
        self.edit("Join the pieces back together", None);
        let Some(restored) = self.scene.restore_split(id) else {
            self.history.discard_last();
            self.status = Status::Warning("The object this was made from could not be rebuilt".into());
            return;
        };
        let node = self.scene.node(restored);
        let (name, kind) = (node.name.clone(), node.kind_label());
        self.collapsed.remove(&restored);
        self.select_only(restored);
        self.status = Status::Info(format!("Joined {pieces} pieces back into {name}, the {kind} they came from"));
    }
}
