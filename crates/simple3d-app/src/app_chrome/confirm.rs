//! Confirmations asked before irreversible actions.

use super::*;
use crate::app::{App, Modal};
use crate::theme;
use crate::ui;

impl App {
    /// Extracting every piece of a collection, which makes a break irreversible (issue 82).
    pub(super) fn confirm_extract_all_window(&mut self, ctx: &egui::Context) {
        self.dialog(
            ctx,
            DialogSpec {
                key: "dialog-confirm-extract-all",
                title: "Extract every piece",
                size: egui::vec2(460.0, 150.0),
                resizable: false,
                fit_height: true,
                min_size: None,
            },
            Self::confirm_extract_all_body,
            Self::confirm_extract_all_actions,
        );
    }

    pub(super) fn confirm_extract_all_body(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.confirm_extract.filter(|&id| self.scene.is_collection(id)) else {
            ui.label("There is nothing left to extract.");
            return;
        };
        let node = self.scene.node(id);
        let count = node.children.len();
        let name = node.name.clone();
        // The original kind is named, since that is what the user will look for; the collection already
        // carries the shape's own name.
        let made_from = node
            .split_original()
            .map(|original| {
                let kind = simple3d_core::primitive::lookup(&original.type_id)
                    .map_or(original.type_id.as_str(), |spec| spec.label)
                    .to_lowercase();
                if original.name == name {
                    format!("the {kind} it was made from")
                } else {
                    format!("{} ({kind})", original.name)
                }
            })
            .unwrap_or_else(|| "the object it was made from".to_string());
        ui.label(format!("Extract all {count} {} of {name}?", if count == 1 { "piece" } else { "pieces" }));
        ui.add_space(6.0);
        ui.label(theme::hint(format!(
            "With nothing left inside it, {name} becomes an ordinary union group and {made_from} is let go: the pieces can no longer be joined back together. Extracting only some of them keeps it."
        )));
    }

    pub(super) fn confirm_extract_all_actions(&mut self, ui: &mut egui::Ui) {
        if ui::dialog_button(ui, "Extract all", true).clicked() {
            let id = self.confirm_extract.take();
            self.modal = Modal::None;
            if let Some(id) = id {
                self.extract_all_pieces(id);
            }
        }
        cancel_at_left(ui, |ui| {
            if ui::dialog_button(ui, "Cancel", true).clicked() {
                self.confirm_extract = None;
                self.modal = Modal::None;
            }
        });
    }
}

impl App {
    /// Deleting a saved pattern kind (issue 67). Asked because it deletes a config file that undo
    /// cannot restore, and the cross sits close to other names.
    pub(super) fn confirm_delete_kind_window(&mut self, ctx: &egui::Context) {
        self.dialog(
            ctx,
            DialogSpec {
                key: "dialog-confirm-delete-kind",
                title: "Delete pattern kind",
                size: egui::vec2(440.0, 150.0),
                resizable: false,
                fit_height: true,
                min_size: None,
            },
            Self::confirm_delete_kind_body,
            Self::confirm_delete_kind_actions,
        );
    }

    pub(super) fn confirm_delete_kind_body(&mut self, ui: &mut egui::Ui) {
        let Some(entry) = self.confirm_delete_kind.clone() else {
            ui.label("There is nothing left to delete.");
            return;
        };
        ui.label(format!("Delete \u{201C}{}\u{201D} from the shelf?", entry.name));
        ui.add_space(6.0);
        // Patterns store their own stage numbers, so existing layouts survive.
        ui.label(theme::hint(
            "The rule goes for good -- this is a file, not an edit, so undo does not bring it back. Patterns already \
             built from it keep their numbers and go on laying out exactly as they do now.",
        ));
    }

    pub(super) fn confirm_delete_kind_actions(&mut self, ui: &mut egui::Ui) {
        if ui::dialog_button(ui, "Delete", true).clicked() {
            let entry = self.confirm_delete_kind.take();
            self.modal = Modal::None;
            if let Some(entry) = entry {
                self.delete_saved_kind(&entry);
            }
        }
        cancel_at_left(ui, |ui| {
            if ui::dialog_button(ui, "Cancel", true).clicked() {
                self.confirm_delete_kind = None;
                self.modal = Modal::None;
            }
        });
    }
}

impl App {
    /// Deleting a component, or undoing its creation after it was worked on (issue 113). Neither can
    /// be undone, since components are outside any one component's history.
    pub(super) fn confirm_component_window(&mut self, ctx: &egui::Context) {
        let title = match self.component_ask {
            Some(crate::components::ComponentAsk::Undo(_)) => "Undo making a component",
            _ => "Delete component",
        };
        self.dialog(
            ctx,
            DialogSpec {
                key: "dialog-confirm-component",
                title,
                size: egui::vec2(460.0, 150.0),
                resizable: false,
                fit_height: true,
                min_size: None,
            },
            Self::confirm_component_body,
            Self::confirm_component_actions,
        );
    }

    pub(super) fn confirm_component_body(&mut self, ui: &mut egui::Ui) {
        use crate::components::ComponentAsk;
        let Some(ask) = self.component_ask else {
            ui.label("There is nothing left to ask about.");
            return;
        };
        let (ComponentAsk::Undo(id) | ComponentAsk::Delete(id)) = ask;
        let name = self.component_label(id).unwrap_or_default();
        let used = self.integration_count(id);
        match ask {
            ComponentAsk::Undo(_) => {
                ui.label(format!("Undoing this takes the component {name} away again."));
                ui.add_space(6.0);
                ui.label(theme::hint(
                    "Everything done to it since it was made is discarded with it, and every other place it is \
                     used is emptied. Redo brings it back as it is now.",
                ));
            }
            ComponentAsk::Delete(_) => {
                ui.label(format!("Delete the component {name}?"));
                ui.add_space(6.0);
                let places = match used {
                    0 => "It is not used anywhere.".to_string(),
                    1 => "The one place it is used goes with it.".to_string(),
                    n => format!("All {n} places it is used go with it."),
                };
                ui.label(theme::hint(format!("{places} This cannot be undone.")));
            }
        }
    }

    pub(super) fn confirm_component_actions(&mut self, ui: &mut egui::Ui) {
        let verb = match self.component_ask {
            Some(crate::components::ComponentAsk::Undo(_)) => "Undo",
            _ => "Delete",
        };
        if ui::dialog_button(ui, verb, true).clicked() {
            self.confirm_component_ask();
        }
        cancel_at_left(ui, |ui| {
            if ui::dialog_button(ui, "Cancel", true).clicked() {
                self.cancel_component_ask();
            }
        });
    }
}
