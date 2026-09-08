//! The keymap editor.

use super::*;
use crate::app::{App, Status};
use crate::theme;
use crate::ui;
use simple3d_core::keymap::{Area, Command, Keymap, MouseButton, Preset};

impl App {
    pub(super) fn keymap_body(&mut self, ui: &mut egui::Ui) {
        // Recording swallows the next key press, so it cannot also fire the
        // command it is being bound to. The keys are read from *this* window's
        // context: the dialog is a window of its own now, and the press that is
        // being bound is delivered to whichever window has the keyboard.
        if let Some(command) = self.recording {
            let (escaped, modifiers, held) = ui.input(|input| {
                let escaped = input
                    .events
                    .iter()
                    .any(|event| matches!(event, egui::Event::Key { key: egui::Key::Escape, pressed: true, .. }));
                (escaped, input.modifiers, ui::keys_down(input))
            });
            // Whatever is held down together is the binding, and it is taken
            // when the hand comes off it: a modifier on its own, an ordinary key,
            // Ctrl+S, or Q+W+E (issues 77 and the follow-up to it). Waiting for
            // the release is what makes the last of those possible at all --
            // taking the first key press could never see the two after it.
            let captured = if escaped {
                self.recording = None;
                self.record_mods.reset();
                None
            } else {
                self.record_mods.update(modifiers, held.iter().map(String::as_str), false)
            };
            if let Some(chord) = captured {
                match self.keymap.set(command, chord.clone(), false) {
                    Ok(()) => {
                        self.recording = None;
                        self.persist_keymap();
                    }
                    // Name the command currently holding it and offer to
                    // reassign or cancel; never overwrite silently.
                    Err(holder) => {
                        self.keymap_conflict = Some((command, chord, holder));
                        self.recording = None;
                    }
                }
            }
        }

        // The window is wider than the contents used to claim, which left every
        // row bunched against the left edge with a band of empty window beside
        // it. The rows are laid out to the width there actually is: one label
        // column for both grids, and the command list below spreading its
        // binding and Reset buttons out to the right-hand edge (issue 63).
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
        // The bindings sit at the right-hand edge and the command name takes
        // whatever is left, so the list is as wide as the window rather than a
        // narrow column with the rest of the window empty beside it.
        let binding_column = 190.0_f32;
        let reset_column = 72.0_f32;
        let name_column = (full - binding_column - reset_column - 48.0).max(160.0);
        let (area, restore) = theme::list_scroll_area(ui);
        area.show(ui, |ui| {
            ui.set_style(restore);
            // One grid for every area rather than one each: a grid measures its
            // own columns, so a grid per area put each area's bindings at its
            // own indent and the buttons down the list did not line up with one
            // another (issue 63). The area names are rows of this one grid.
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
                        let recording = self.recording == Some(command);
                        let text = if recording {
                            // Modifiers are keys too now (issue 77), and so is
                            // any set of keys held together, so the prompt says
                            // what it takes rather than leaving someone waiting
                            // for a single letter to be required.
                            "hold the keys, then let go...".to_string()
                        } else {
                            let shown = self.keymap.shortcut_text(command);
                            if shown.is_empty() {
                                "unbound".to_string()
                            } else {
                                shown
                            }
                        };
                        let button = egui::vec2(binding_column, theme::metric::INPUT_ROW);
                        if ui.add(egui::Button::new(text).min_size(button)).clicked() {
                            self.recording = Some(command);
                            // The click itself may have been made with a modifier
                            // down; that hold is not the binding.
                            self.record_mods.reset();
                        }
                        let reset = egui::vec2(reset_column, theme::metric::INPUT_ROW);
                        if ui.add(egui::Button::new("Reset").min_size(reset)).clicked() {
                            self.keymap.reset(command);
                            self.persist_keymap();
                        }
                        ui.end_row();
                    }
                    ui.end_row();
                }
            });
        });

        // Drawn over the keymap dialog, inside it: a question about the key that
        // was just pressed belongs to the window that took the press.
        if let Some((command, chord, holder)) = self.keymap_conflict.clone() {
            egui::Window::new("That combination is already in use")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 40.0))
                .show(ui.ctx(), |ui| {
                    ui.label(format!("{chord} is currently bound to \"{}\".", holder.label()));
                    ui.label(format!("Reassign it to \"{}\"?", command.label()));
                    ui.horizontal(|ui| {
                        if ui.button("Reassign").clicked() {
                            let _ = self.keymap.set(command, chord.clone(), true);
                            self.persist_keymap();
                            self.keymap_conflict = None;
                        }
                        if ui.button("Cancel").clicked() {
                            self.keymap_conflict = None;
                        }
                    });
                });
        }
    }
}
