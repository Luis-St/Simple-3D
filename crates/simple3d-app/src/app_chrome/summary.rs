//! How a selection is described in a line.

use crate::app::{App, Modal};
use crate::ui;

impl App {
    /// "2 selected", or the name when there is exactly one -- the name is more
    /// use than the count when the count is one.
    pub(crate) fn selection_summary(&self) -> String {
        match self.selection.len() {
            0 => "Nothing selected".to_string(),
            // Asked of the scene rather than taken from the selection: an id the
            // scene no longer holds is a panic from `Scene::node`, and this is
            // drawn on every frame of the status bar -- so it would be the first
            // thing to run after whatever left the id behind, and would take the
            // window down before anything could prune it.
            1 => match self.scene.get(self.selection[0]) {
                Some(node) => node.name.clone(),
                None => "Nothing selected".to_string(),
            },
            n => format!("{n} selected"),
        }
    }

    /// The bounding size of what is selected, or of the whole scene when nothing
    /// is -- the status bar's answer to "will this fit".
    pub(crate) fn selection_size_text(&self) -> String {
        let unit = self.unit();
        let bounds = match self.primary() {
            Some(id) => self.evaluated.node_world_bounds.get(&id).copied(),
            None => self.evaluated.mesh.bounds(),
        };
        match bounds {
            Some((lo, hi)) => ui::describe_size(hi - lo, unit),
            None => format!("-- {}", unit.suffix()),
        }
    }

    pub(crate) fn modals(&mut self, ctx: &egui::Context) {
        match self.modal {
            // Nothing open, so the next dialog to open is placed afresh.
            Modal::None => self.dialog_placed = None,
            Modal::Export => self.export_window(ctx),
            Modal::Keymap => self.keymap_window(ctx),
            Modal::About => self.about_window(ctx),
            Modal::Error => self.error_window(ctx),
            Modal::ConfirmQuit => self.confirm_quit_window(ctx),
            Modal::ConfirmCloseTab => self.confirm_close_tab_window(ctx),
            Modal::SavePrimitive => self.save_primitive_window(ctx),
            Modal::PatternKind => self.pattern_kind_window(ctx),
            Modal::ConfirmExtractAll => self.confirm_extract_all_window(ctx),
        }
    }
}
