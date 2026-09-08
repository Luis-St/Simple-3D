//! Saving a selection to the user's own library.

use super::*;
use crate::app::App;
use crate::theme;
use crate::ui;

impl App {
    /// Naming a group, or a whole project, before it goes on the palette.
    ///
    /// A window rather than an inline field because the name is going into the
    /// user's library, not into the document: it outlives this project, and it
    /// is the only thing the palette will show, so it is worth stopping to type.
    pub(super) fn save_primitive_window(&mut self, ctx: &egui::Context) {
        self.dialog(
            ctx,
            DialogSpec {
                key: "dialog-save-primitive",
                title: "Save as primitive",
                size: egui::vec2(480.0, 200.0),
                resizable: false,
                fit_height: false,
                min_size: None,
            },
            Self::save_primitive_body,
            Self::save_primitive_actions,
        );
    }

    pub(super) fn save_primitive_body(&mut self, ui: &mut egui::Ui) {
        let count = self.primitive_clip.as_ref().map(|c| c.nodes.len()).unwrap_or(0);
        ui.label(format!(
            "{count} node{} will be kept on the palette, ready to drop into any project.",
            if count == 1 { "" } else { "s" }
        ));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Name");
            let field =
                ui.add(egui::TextEdit::singleline(&mut self.primitive_name).desired_width(240.0).hint_text("Bracket"));
            field.request_focus();
            if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.confirm_save_primitive();
            }
        });
        let tidied = simple3d_core::library::sanitise(&self.primitive_name);
        if tidied.is_empty() {
            ui.add(egui::Label::new(theme::hint("A saved primitive needs a name.")).selectable(false));
        } else if tidied != self.primitive_name.trim() {
            ui.add(
                egui::Label::new(theme::hint(format!("It will be saved as \u{201C}{tidied}\u{201D}.")))
                    .selectable(false),
            );
        } else if simple3d_core::library::exists(self.config_dir(), &tidied) {
            ui.add(
                egui::Label::new(theme::hint(format!(
                    "\u{201C}{tidied}\u{201D} is already on the palette; saving replaces it."
                )))
                .selectable(false),
            );
        }
    }

    pub(super) fn save_primitive_actions(&mut self, ui: &mut egui::Ui) {
        let named = !simple3d_core::library::sanitise(&self.primitive_name).is_empty();
        if ui::dialog_button(ui, "Save", named).clicked() {
            self.confirm_save_primitive();
        }
        if ui::dialog_button(ui, "Cancel", true).clicked() {
            self.cancel_save_primitive();
        }
    }
}
