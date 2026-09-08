//! The questions asked before something cannot be taken back.

use super::*;
use crate::app::{App, Modal};
use crate::theme;
use crate::ui;

impl App {
    /// Emptying a collection of every piece, which is where a break stops being
    /// reversible (issue 82).
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
        // The recipe is named, not merely referred to: "you will lose the
        // original" is a warning about something the user cannot see, and what
        // the shape *was* is the thing the user will look for afterwards. The
        // collection wears the shape's own name, so naming it again would be
        // the same word twice: it is the kind that says something new.
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
    pub(super) fn confirm_close_tab_window(&mut self, ctx: &egui::Context) {
        self.dialog(
            ctx,
            DialogSpec {
                key: "dialog-confirm-close-tab",
                title: "Unsaved changes",
                size: egui::vec2(440.0, 150.0),
                resizable: false,
                fit_height: false,
                min_size: None,
            },
            Self::confirm_close_tab_body,
            Self::confirm_close_tab_actions,
        );
    }

    pub(super) fn confirm_close_tab_body(&mut self, ui: &mut egui::Ui) {
        let name = self.pending_close.map(|index| self.tab_summary(index).0).unwrap_or_default();
        ui.label(format!("{name} has changes that have not been saved."));
        ui.add_space(4.0);
        ui.add(
            egui::Label::new(theme::hint("Saving writes it to its file; closing without saving discards it."))
                .selectable(false),
        );
    }

    pub(super) fn confirm_close_tab_actions(&mut self, ui: &mut egui::Ui) {
        if ui::dialog_button(ui, "Save and close", true).clicked() {
            self.save_and_close_tab();
        }
        if ui::dialog_button(ui, "Close without saving", true).clicked() {
            self.confirm_close_tab();
        }
        cancel_at_left(ui, |ui| {
            if ui::dialog_button(ui, "Cancel", true).clicked() {
                self.cancel_close_tab();
            }
        });
    }
}

impl App {
    pub(super) fn confirm_quit_window(&mut self, ctx: &egui::Context) {
        self.dialog(
            ctx,
            DialogSpec {
                key: "dialog-confirm-quit",
                title: "Unsaved changes",
                size: egui::vec2(500.0, 150.0),
                resizable: false,
                fit_height: false,
                min_size: None,
            },
            Self::confirm_quit_body,
            Self::confirm_quit_actions,
        );
    }

    pub(super) fn confirm_quit_body(&mut self, ui: &mut egui::Ui) {
        let others = (0..self.tab_count()).filter(|i| self.tab_summary(*i).1).count().saturating_sub(1);
        ui.label(match others {
            0 => "This project has changes that have not been saved.".to_string(),
            1 => "This project, and one other open document, have changes that have not been saved.".to_string(),
            n => format!("This project, and {n} other open documents, have changes that have not been saved."),
        });
        ui.add_space(4.0);
        ui.add(egui::Label::new(theme::hint("Only the document on screen can be saved from here.")).selectable(false));
    }

    pub(super) fn confirm_quit_actions(&mut self, ui: &mut egui::Ui) {
        if ui::dialog_button(ui, "Save and quit", true).clicked() {
            self.save();
            if !self.unsaved() {
                self.confirm_quit();
            }
            self.modal = Modal::None;
        }
        if ui::dialog_button(ui, "Quit without saving", true).clicked() {
            self.confirm_quit();
            self.modal = Modal::None;
        }
        cancel_at_left(ui, |ui| {
            if ui::dialog_button(ui, "Cancel", true).clicked() {
                self.modal = Modal::None;
            }
        });
    }
}
