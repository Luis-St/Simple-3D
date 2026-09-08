//! The menu bar itself, and one command on it.

use crate::app::App;
use crate::theme;
use crate::ui;
use simple3d_core::keymap::Command;

impl App {
    /// The menu bar.
    ///
    /// Only the menus and the document's name: the window system draws the
    /// title bar above this row, so there are no window buttons to place and
    /// nothing here drags the window. What used to be drawn by hand -- the
    /// buttons, the drag, the eight resize grips -- is the compositor's again,
    /// which is where the snapping, the window menu and the user's own button
    /// layout come from.
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
                // The document's name at the far end: what is open, and whether
                // it still matches what is on disk. The title bar carries it
                // too, but a title bar can be off the top of a maximised
                // screen's attention while this row never is.
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
