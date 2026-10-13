//! The keymap editor.

use super::*;
use crate::app::{App, Status};
use crate::theme;
use simple3d_core::keymap::{Action, Area, Command, Keymap, MouseButton, Preset};

impl App {
    pub(super) fn keymap_body(&mut self, ui: &mut egui::Ui) {
        // Recording swallows the next key press so it cannot fire the command being bound. Keys are read
        // from this window's context, since the dialog is its own window.
        self.record_chord(ui.ctx());

        // Rows are laid out to the actual width, with one label column for both grids and buttons at
        // the right edge (issue 63).
        let full = ui.available_width();
        let label_column = 130.0_f32;
        egui::Grid::new("keymap-top").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            label_cell(ui, "Preset", label_column);
            ui.horizontal(|ui| {
                let mut preset = self.keymap.preset;
                egui::ComboBox::from_id_salt("keymap-preset").selected_text(preset.label()).width(190.0).show_ui(
                    ui,
                    |ui| {
                        for option in Preset::ALL {
                            ui.selectable_value(&mut preset, option, option.label());
                        }
                    },
                );
                if preset != self.keymap.preset {
                    // A preset is a starting point the user can then modify.
                    self.keymap.switch_preset(preset);
                    self.persist_keymap();
                    self.status = Status::Info(format!("Keymap preset: {}", preset.label()));
                }
                if ui.button("Reset everything to the preset").clicked() {
                    self.keymap.reset_all();
                    self.persist_keymap();
                }
            });
            ui.end_row();

            label_cell(ui, "Keymap file", label_column);
            ui.horizontal(|ui| {
                if ui.button("Export...").clicked() {
                    let dialog = rfd::FileDialog::new()
                        .add_filter("Simple 3D keymap", &["json"])
                        .set_file_name("simple3d-keymap.json");
                    self.ask_for_file("Keymap export", dialog, true, |app, path| {
                        if let Err(e) = std::fs::write(&path, app.keymap.to_text()) {
                            app.fail("Could not write the keymap", &e.to_string());
                        }
                    });
                }
                if ui.button("Import...").clicked() {
                    let dialog = rfd::FileDialog::new().add_filter("Simple 3D keymap", &["json"]);
                    self.ask_for_file("Keymap import", dialog, false, |app, path| {
                        match std::fs::read_to_string(&path)
                            .map_err(|e| e.to_string())
                            .and_then(|t| Keymap::from_text(&t))
                        {
                            Ok(keymap) => {
                                app.keymap = keymap;
                                app.persist_keymap();
                            }
                            Err(e) => app.fail("Could not read the keymap", &e),
                        }
                    });
                }
            });
            ui.end_row();
        });
        ui.separator();

        ui.add(egui::Label::new(theme::header_text("Navigation")).selectable(false));
        egui::Grid::new("nav-grid").num_columns(3).spacing([12.0, 8.0]).show(ui, |ui| {
            let mut nav = self.keymap.nav;
            for (label, drag) in [("Orbit", 0), ("Pan", 1)] {
                label_cell(ui, label, label_column);
                let binding = if drag == 0 { &mut nav.orbit } else { &mut nav.pan };
                egui::ComboBox::from_id_salt(format!("nav-button-{drag}"))
                    .selected_text(binding.button.label())
                    .width(90.0)
                    .show_ui(ui, |ui| {
                        for button in MouseButton::ALL {
                            ui.selectable_value(&mut binding.button, button, button.label());
                        }
                    });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut binding.ctrl, "Ctrl");
                    ui.checkbox(&mut binding.shift, "Shift");
                    ui.checkbox(&mut binding.alt, "Alt");
                });
                ui.end_row();
            }
            label_cell(ui, "Zoom wheel", label_column);
            ui.checkbox(&mut nav.invert_zoom, "Inverted");
            ui.label("");
            ui.end_row();
            if nav != self.keymap.nav {
                // Applies immediately, without a restart.
                self.keymap.nav = nav;
                self.persist_keymap();
            }
        });
        if self.keymap.nav.orbit.button == self.keymap.nav.pan.button
            && self.keymap.nav.orbit.ctrl == self.keymap.nav.pan.ctrl
            && self.keymap.nav.orbit.shift == self.keymap.nav.pan.shift
            && self.keymap.nav.orbit.alt == self.keymap.nav.pan.alt
        {
            ui.colored_label(ui.visuals().warn_fg_color, "Orbit and pan are on the same binding; pan will never fire.");
        }

        ui.separator();
        ui.horizontal(|ui| {
            label_cell(ui, "Search", label_column);
            let clear = 64.0;
            ui.add(
                egui::TextEdit::singleline(&mut self.keymap_search)
                    .desired_width((full - label_column - clear - 32.0).max(120.0))
                    .hint_text("Filter by name"),
            );
            if ui.add(egui::Button::new("Clear").min_size(egui::vec2(clear, 0.0))).clicked() {
                self.keymap_search.clear();
            }
        });

        let needle = self.keymap_search.to_lowercase();
        // Bindings at the right edge, the command name takes the rest.
        let binding_column = 190.0_f32;
        let reset_column = 72.0_f32;
        let name_column = (full - binding_column - reset_column - 48.0).max(160.0);
        let (area, restore) = theme::list_scroll_area(ui);
        area.show(ui, |ui| {
            ui.set_style(restore);
            // One grid for all areas so the buttons line up (issue 63); area names are rows of it.
            egui::Grid::new("keymap-commands").num_columns(3).spacing([12.0, 6.0]).show(ui, |ui| {
                for area in Area::ALL {
                    let commands: Vec<Command> = Command::ALL
                        .iter()
                        .copied()
                        .filter(|c| c.area() == area)
                        .filter(|c| needle.is_empty() || c.label().to_lowercase().contains(&needle))
                        .collect();
                    if commands.is_empty() {
                        continue;
                    }
                    ui.add(egui::Label::new(theme::header_text(area.label())).selectable(false));
                    ui.end_row();
                    for command in commands {
                        label_cell(ui, command.label(), name_column);
                        let shown = self.keymap.shortcut_text(command);
                        self.binding_button(ui, Action::Command(command), shown, binding_column);
                        let reset = egui::vec2(reset_column, theme::metric::INPUT_ROW);
                        if ui.add(egui::Button::new("Reset").min_size(reset)).clicked() {
                            self.keymap.reset(command);
                            self.persist_keymap();
                        }
                        ui.end_row();
                    }
                    ui.end_row();
                }
                self.primitive_rows(ui, &needle, name_column, binding_column, reset_column);
            });
        });

        // Drawn inside the keymap dialog, the window that took the key press.
        if let Some((target, chord, holder)) = self.keymap_conflict.clone() {
            egui::Window::new("That combination is already in use")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 40.0))
                .show(ui.ctx(), |ui| {
                    ui.label(format!("{chord} is currently bound to \"{}\".", holder.label()));
                    ui.label(format!("Reassign it to \"{}\"?", target.label()));
                    ui.horizontal(|ui| {
                        if ui.button("Reassign").clicked() {
                            self.bind_recorded(target.clone(), chord.clone(), true);
                        }
                        if ui.button("Cancel").clicked() {
                            self.keymap_conflict = None;
                        }
                    });
                });
        }
    }
}

impl App {
    /// The button showing `target`'s binding; a click records a new one.
    fn binding_button(&mut self, ui: &mut egui::Ui, target: Action, shown: String, width: f32) {
        let text = if self.recording.as_ref() == Some(&target) {
            // Modifiers and multi-key holds are bindings too (issue 77), so the prompt says so.
            "hold the keys, then let go...".to_string()
        } else if shown.is_empty() {
            "unbound".to_string()
        } else {
            shown
        };
        if ui.add(egui::Button::new(text).min_size(egui::vec2(width, theme::metric::INPUT_ROW))).clicked() {
            self.start_recording(target);
        }
    }

    /// Shortcuts adding each primitive (issue 124), after the commands; none are bound by a preset,
    /// so their second button clears rather than resets.
    fn primitive_rows(
        &mut self,
        ui: &mut egui::Ui,
        needle: &str,
        name_column: f32,
        binding_column: f32,
        clear_column: f32,
    ) {
        let specs: Vec<_> = simple3d_core::primitive::REGISTRY
            .iter()
            .filter(|spec| needle.is_empty() || format!("add {}", spec.label).to_lowercase().contains(needle))
            .collect();
        if specs.is_empty() {
            return;
        }
        ui.add(egui::Label::new(theme::header_text("Primitives")).selectable(false));
        ui.end_row();
        for spec in specs {
            label_cell(ui, &format!("Add {}", spec.label), name_column);
            let shown = self.keymap.primitive_shortcut_text(spec.type_id);
            self.binding_button(ui, Action::Primitive(spec.type_id.to_string()), shown, binding_column);
            let bound = self.keymap.primitive_binding(spec.type_id).is_some();
            let clear = egui::vec2(clear_column, theme::metric::INPUT_ROW);
            if ui.add_enabled(bound, egui::Button::new("Clear").min_size(clear)).clicked() {
                self.keymap.unbind_primitive(spec.type_id);
                self.persist_keymap();
            }
            ui.end_row();
        }
    }
}
