//! The File and Edit menus.

use crate::app::{App, Modal};
use crate::ui;
use simple3d_core::config::OpenTarget;
use simple3d_core::keymap::Command;

impl App {
    pub(super) fn file_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("File", |ui| {
            self.command_item(ui, Command::New, true);
            self.command_item(ui, Command::Open, true);

            let recent = self.settings.recent_files.clone();
            ui.add_enabled_ui(!recent.is_empty(), |ui| {
                ui.menu_button("Open recent", |ui| {
                    for path in &recent {
                        let label = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                        if ui.button(label).on_hover_text(path.display().to_string()).clicked() {
                            self.open_path(path);
                            ui.close();
                        }
                    }
                    ui.separator();
                    if ui.button("Clear the list").clicked() {
                        self.settings.recent_files.clear();
                        ui.close();
                    }
                });
            });

            // Where an opened model goes when there is already one open
            // (issue 107): beside the two entries that open one, which is where
            // the question comes up.
            ui.menu_button("Open a model in", |ui| {
                for option in OpenTarget::ALL {
                    let chosen = self.settings.open_target == option;
                    let label = format!("{} {}", if chosen { "*" } else { " " }, option.label());
                    if ui::menu_entry(ui, &label, true).on_hover_text(option.description()).clicked() {
                        self.settings.open_target = option;
                        ui.close();
                    }
                }
            });

            ui.separator();
            self.command_item(ui, Command::CloseTab, true);
            self.command_item(ui, Command::NextTab, self.tab_count() > 1);
            self.command_item(ui, Command::PreviousTab, self.tab_count() > 1);
            // Closing the window rather than the document in it. With one window
            // open it is the same thing as Quit and asks the same question; with
            // more than one it leaves the others where they are.
            if ui.button("Close window").on_hover_text("Close this window and the documents in it").clicked() {
                self.request_close_window();
                ui.close();
            }
            ui.separator();
            self.command_item(ui, Command::Save, true);
            self.command_item(ui, Command::SaveAs, true);
            ui.separator();
            // Beside Export, which it is the other half of (issue 105).
            self.command_item(ui, Command::Import, true);
            self.command_item(ui, Command::Export, true);
            ui.separator();
            let has_selection = !self.selection.is_empty();
            if ui
                .add_enabled(has_selection, egui::Button::new("Save selection as primitive\u{2026}"))
                .on_hover_text("Keep the selection on the palette, to use in any project")
                .clicked()
            {
                self.save_selection_as_primitive();
                ui.close();
            }
            let has_content = !self.scene.node(self.scene.root()).children.is_empty();
            if ui
                .add_enabled(has_content, egui::Button::new("Save project as primitive\u{2026}"))
                .on_hover_text("Keep the whole document on the palette, to use in any project")
                .clicked()
            {
                self.save_project_as_primitive();
                ui.close();
            }
            ui.separator();
            self.command_item(ui, Command::Quit, true);
        });
    }

    pub(super) fn edit_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Edit", |ui| {
            let undo = self.history.undo_label().map(|l| format!("Undo {l}"));
            let redo = self.history.redo_label().map(|l| format!("Redo {l}"));
            let can_undo = self.history.can_undo();
            let can_redo = self.history.can_redo();
            let undo_text =
                format!("{}\t{}", undo.unwrap_or_else(|| "Undo".into()), self.keymap.shortcut_text(Command::Undo));
            let redo_text =
                format!("{}\t{}", redo.unwrap_or_else(|| "Redo".into()), self.keymap.shortcut_text(Command::Redo));
            if ui::menu_entry(ui, &undo_text, can_undo).clicked() {
                self.run(Command::Undo);
                ui.close();
            }
            if ui::menu_entry(ui, &redo_text, can_redo).clicked() {
                self.run(Command::Redo);
                ui.close();
            }
            ui.separator();
            let has_selection = !self.selection.is_empty();
            self.command_item(ui, Command::Copy, has_selection);
            self.command_item(ui, Command::Cut, has_selection);
            self.command_item(ui, Command::Paste, self.clipboard.is_some());
            self.command_item(ui, Command::Duplicate, has_selection);
            self.command_item(ui, Command::Delete, has_selection);
            ui.separator();
            // The same blocks the outliner's own menu is divided into, in the
            // same order (issue 94): the two containers, then what a node can
            // be turned into, then the row itself.
            self.command_item(ui, Command::Group, has_selection);
            // Beside Group, which is the command it is a variant of. It works
            // with nothing selected too -- an empty pattern to fill later -- so
            // unlike Group it is never disabled, and says which of the two it
            // is about to do: "of the selection" with nothing selected read as
            // an entry left enabled by mistake.
            if has_selection {
                self.command_item(ui, Command::Pattern, true);
            } else {
                let label = ui::menu_label(&self.keymap, Command::Pattern).replacen(
                    Command::Pattern.label(),
                    super::EMPTY_PATTERN,
                    1,
                );
                if ui::menu_entry(ui, &label, true).clicked() {
                    self.run(Command::Pattern);
                    ui.close();
                }
            }
            ui.separator();
            // Baking a shape into the triangles it evaluates to (issue 80), and
            // cutting one into a pattern of pieces (issue 82).
            self.command_item(ui, Command::ConvertToMesh, !self.selection.is_empty());
            // Enabled only on a mesh: it is the only body made of triangles
            // rather than of a recipe, and so the only one with triangles to
            // drop (issue 106).
            self.command_item(
                ui,
                Command::SimplifyMesh,
                self.selection.len() == 1 && self.primary().is_some_and(|id| self.scene.node(id).is_mesh()),
            );
            // The way back from the conversion, and so enabled on the same one
            // row it is (issue 108).
            self.command_item(
                ui,
                Command::Reassemble,
                self.selection.len() == 1 && self.primary().is_some_and(|id| self.scene.node(id).is_mesh()),
            );
            self.command_item(ui, Command::SplitIntoPieces, self.selection.len() == 1);
            // Enabled only on a split, because a split is the only thing it has
            // anything to say to -- everything else was never cut up.
            self.command_item(
                ui,
                Command::Rejoin,
                self.selection.len() == 1 && self.primary().is_some_and(|id| self.scene.node(id).is_split()),
            );
            ui.separator();
            self.command_item(ui, Command::Rename, has_selection);
            self.command_item(ui, Command::ToggleVisibility, has_selection);
            self.command_item(ui, Command::MoveUp, self.can_reorder(-1));
            self.command_item(ui, Command::MoveDown, self.can_reorder(1));
            ui.separator();
            if ui.button("Keyboard and mouse...").clicked() {
                self.modal = Modal::Keymap;
                ui.close();
            }
        });
    }
}
