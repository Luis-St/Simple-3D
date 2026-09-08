//! The small dialogs: about, errors, and the ones a tool opens.

use super::*;
use crate::app::{App, Modal, APP_NAME, PROJECT_EXTENSION, VERSION};
use crate::theme;
use crate::ui;
use simple3d_core::config::{self};

impl App {
    /// The keymap editor: every command grouped by area, with a search box, the
    /// current binding shown, and click-to-record (spec section 8.2).
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
        // Nothing but text, of a length that depends on where this machine
        // keeps its settings, so the window is exactly as tall as the lines
        // turn out to be: no band of empty surface under the last of them, and
        // no line cut off at the bottom either.
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
    /// Failures are shown in a scrollable, copyable window with the specific
    /// reason, never a generic message (spec section 9).
    pub(super) fn error_window(&mut self, ctx: &egui::Context) {
        // A window with no title in its bar reads as a broken window; every
        // failure names itself, but nothing here depends on that.
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
        // The field fills the window: a failure is often a path and a system
        // message, and the room to read it is the point of the window.
        let height = ui.available_height().max(120.0);
        let (area, restore) = theme::list_scroll_area(ui);
        area.show(ui, |ui| {
            ui.set_style(restore);
            // A read-only multiline field, so the text can be selected
            // and copied. Sized to the room the window has rather than to a
            // row count, so the field is the window and not a box in it.
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

impl App {
    /// The custom pattern kind creation tool (issue 67).
    pub(super) fn pattern_kind_window(&mut self, ctx: &egui::Context) {
        // Most of the parent window rather than a fixed 820 x 520, which was a
        // window every user resized before doing anything else: half of this one
        // is a viewport, and a viewport the size of a postage stamp is a picture
        // of a pattern rather than a look at one. Bounded so it neither shrinks
        // below the two columns nor runs off a small screen.
        let parent = ctx.input(|i| i.viewport().outer_rect.map(|rect| rect.size()));
        let size = match parent {
            Some(size) => egui::vec2((size.x * 0.82).clamp(900.0, 1500.0), (size.y * 0.82).clamp(560.0, 980.0)),
            None => egui::vec2(1100.0, 720.0),
        };
        self.dialog(
            ctx,
            DialogSpec {
                key: "dialog-pattern-kind",
                title: "Custom pattern kind",
                size,
                resizable: true,
                fit_height: false,
                // The tool gives the picture up as the window narrows and ends
                // as a single column, but a stage still has a name and a field
                // on every row and a button row still has two buttons in it.
                // Below this there is no layout left to find.
                min_size: Some(egui::vec2(320.0, 260.0)),
            },
            crate::pattern_tool::body,
            crate::pattern_tool::actions,
        );
    }
}
