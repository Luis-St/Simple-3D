//! The small dialogs: about, errors, and those a tool opens.

use super::*;
use crate::app::{App, Modal, APP_NAME, PROJECT_EXTENSION, VERSION};
use crate::theme;
use crate::ui;
use simple3d_core::config::{self};

impl App {
    /// The keymap editor: commands by area, a search box, current bindings, and click-to-record (spec
    /// section 8.2).
    pub(super) fn keymap_window(&mut self, ctx: &egui::Context) {
        self.dialog(
            ctx,
            DialogSpec {
                key: "dialog-keymap",
                title: "Keyboard and mouse",
                size: egui::vec2(620.0, 660.0),
                resizable: true,
                fit_height: false,
                min_size: None,
            },
            Self::keymap_body,
            Self::close_action,
        );
    }
}

impl App {
    pub(super) fn about_window(&mut self, ctx: &egui::Context) {
        let title = format!("About {APP_NAME}");
        // Text only, of machine-dependent length, so the window fits its lines exactly.
        self.dialog(
            ctx,
            DialogSpec {
                key: "dialog-about",
                title: &title,
                size: egui::vec2(460.0, 220.0),
                resizable: false,
                fit_height: true,
                min_size: None,
            },
            Self::about_body,
            Self::close_action,
        );
    }

    pub(super) fn about_body(&mut self, ui: &mut egui::Ui) {
        ui.heading(APP_NAME);
        ui.label(format!("Version {VERSION}"));
        ui.add_space(6.0);
        ui.label("Parametric 3D modelling with exact metric dimensions.");
        ui.label("Everything is stored in millimetres; the display unit only changes what you read.");
        ui.add_space(6.0);
        ui.label("PolyForm Noncommercial License 1.0.0: free for any noncommercial purpose.");
        ui.add_space(6.0);
        ui.label(format!("Project files: .{PROJECT_EXTENSION}"));
        ui.label(format!("Settings: {}", self.config_dir().display()));
        if config::portable_mode() {
            ui.label("Running in portable mode: settings live beside the executable.");
        }
    }
}

impl App {
    /// Failures in a scrollable, copyable window with the specific reason (spec section 9).
    pub(super) fn error_window(&mut self, ctx: &egui::Context) {
        // Always titled, since an untitled window looks broken.
        let title =
            if self.error_title.is_empty() { "Something went wrong".to_string() } else { self.error_title.clone() };
        self.dialog(
            ctx,
            DialogSpec {
                key: "dialog-error",
                title: &title,
                size: egui::vec2(560.0, 360.0),
                resizable: true,
                fit_height: false,
                min_size: None,
            },
            Self::error_body,
            Self::error_actions,
        );
    }

    pub(super) fn error_body(&mut self, ui: &mut egui::Ui) {
        let mut detail = self.error_detail.clone();
        // The field fills the window, since the room to read a path and message is the point.
        let height = ui.available_height().max(120.0);
        let (area, restore) = theme::list_scroll_area(ui);
        area.show(ui, |ui| {
            ui.set_style(restore);
            // A read-only multiline field so the text can be copied, sized to the window.
            ui.add_sized(
                egui::vec2(ui.available_width(), height),
                egui::TextEdit::multiline(&mut detail).desired_width(f32::INFINITY).interactive(true),
            );
        });
    }

    pub(super) fn error_actions(&mut self, ui: &mut egui::Ui) {
        if ui::dialog_button(ui, "Close", true).clicked() {
            self.modal = Modal::None;
        }
        if ui::dialog_button(ui, "Copy", true).clicked() {
            ui.ctx().copy_text(self.error_detail.clone());
        }
    }
}
