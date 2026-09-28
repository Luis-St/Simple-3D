//! Confirmations asked before closing a tab or quitting with unsaved work.

use super::*;
use crate::app::{App, Modal};
use crate::theme;
use crate::ui;

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
        // Quitting closes other windows, so their unsaved work is part of the question (issue 107);
        // closing one window is not.
        if !self.closing_one_window() && self.unsaved_elsewhere {
            ui.add_space(4.0);
            ui.label("Another window has unsaved changes as well.");
        }
        ui.add_space(4.0);
        ui.add(egui::Label::new(theme::hint("Only the document on screen can be saved from here.")).selectable(false));
    }

    pub(super) fn confirm_quit_actions(&mut self, ui: &mut egui::Ui) {
        // With one window, closing it is quitting; with others open it is only a window (issue 107).
        let (save, leave) = if self.closing_one_window() {
            ("Save and close", "Close without saving")
        } else {
            ("Save and quit", "Quit without saving")
        };
        if ui::dialog_button(ui, save, true).clicked() {
            self.save();
            if !self.unsaved() {
                self.confirm_quit();
            }
            self.modal = Modal::None;
        }
        if ui::dialog_button(ui, leave, true).clicked() {
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
