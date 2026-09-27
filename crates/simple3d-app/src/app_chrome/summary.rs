//! How a selection is described in a line.

use crate::app::{App, Modal};
use crate::ui;

impl App {
    /// "2 selected", or the name when exactly one is.
    pub(crate) fn selection_summary(&self) -> String {
        match self.selection.len() {
            0 => "Nothing selected".to_string(),
            // Asked of the scene: a stale id panics in `Scene::node`, and this runs every frame, so it would crash
            // the window before the id could be pruned.
            1 => match self.scene.get(self.selection[0]) {
                Some(node) => node.name.clone(),
                None => "Nothing selected".to_string(),
            },
            n => format!("{n} selected"),
        }
    }

    /// Bounding size of the selection, or the whole scene when nothing is: "will this fit".
    pub(crate) fn selection_size_text(&self) -> String {
        let unit = self.unit();
        let bounds = match self.primary() {
            Some(id) => self.evaluated.node_world_bounds.get(&id).copied(),
            None => self.evaluated.bounds,
        };
        match bounds {
            Some((lo, hi)) => ui::describe_size(hi - lo, unit),
            None => format!("-- {}", unit.suffix()),
        }
    }

    pub(crate) fn modals(&mut self, ctx: &egui::Context) {
        match self.modal {
            // Nothing open, so the next dialog is placed afresh.
            Modal::None => self.dialog_placed = None,
            Modal::Export => self.export_window(ctx),
            Modal::Keymap => self.keymap_window(ctx),
            Modal::About => self.about_window(ctx),
            Modal::Error => self.error_window(ctx),
            Modal::ConfirmQuit => self.confirm_quit_window(ctx),
            Modal::ConfirmCloseTab => self.confirm_close_tab_window(ctx),
            Modal::SavePrimitive => self.save_primitive_window(ctx),
            Modal::ConfirmExtractAll => self.confirm_extract_all_window(ctx),
            Modal::ConfirmDeleteKind => self.confirm_delete_kind_window(ctx),
            Modal::ConfirmComponent => self.confirm_component_window(ctx),
        }
    }
}
