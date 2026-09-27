//! The menu bar itself, and one command on it.

use crate::app::App;
use crate::theme;
use crate::ui;
use simple3d_core::keymap::Command;

impl App {
    /// The menu bar: menus and the document name only. The compositor draws the title bar, so window
    /// buttons, dragging and resizing are its (snapping, window menu, button layout) again.
    pub(crate) fn menu_bar(&mut self, ctx: &egui::Context) {
        let frame = egui::Frame::NONE.fill(theme::token::SURFACE_2).inner_margin(egui::Margin {
            left: 8,
            right: 8,
            top: 0,
            bottom: 0,
        });
        egui::TopBottomPanel::top("menu").frame(frame).exact_height(theme::metric::MENU_BAR).show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    self.file_menu(ui);
                    self.edit_menu(ui);
                    self.add_menu(ui);
                    self.view_menu(ui);
                    self.manipulate_menu(ui);
                    self.help_menu(ui);
                });
                // The document name and saved state at the far end; the title bar may be out of view when maximised.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let name = self
                        .path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "Untitled".to_string());
                    let marker = if self.unsaved() { " \u{2022}" } else { "" };
                    ui.add(egui::Label::new(theme::hint(format!("{name}{marker}"))).selectable(false))
                        .on_hover_text(if self.unsaved() { "Unsaved changes" } else { "Saved" });
                });
            });
        });
    }

    pub(super) fn command_item(&mut self, ui: &mut egui::Ui, command: Command, enabled: bool) {
        let label = ui::menu_label(&self.keymap, command);
        if ui::menu_entry(ui, &label, enabled).clicked() {
            self.run(command);
            ui.close();
        }
    }
}
