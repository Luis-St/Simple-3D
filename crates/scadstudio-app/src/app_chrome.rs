//! The window furniture: keyboard dispatch, menu bar, status bar and the modal
//! windows (export, keymap editor, scene settings, about, errors, quit
//! confirmation).

use crate::app::{App, Modal, Status, APP_NAME, PROJECT_EXTENSION, VERSION};
use crate::gizmo::Mode;
use crate::render::Renderable;
use crate::ui;
use scadstudio_core::config::{self, DisplayMode, HandleFrame};
use scadstudio_core::keymap::{Area, Command, Keymap, MouseButton, Preset};
use scadstudio_core::primitive;
use scadstudio_core::scene::{GroupOp, NodeId};
use scadstudio_core::unit::{format_number, Unit};
use scadstudio_export::Format;
use std::hash::{Hash, Hasher};

impl App {
    /// Rebuild the per-node meshes the viewport needs -- the selection outline and
    /// the ghosts -- when either the evaluation or what is selected has changed.
    pub(crate) fn refresh_node_renderables(&mut self) {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.evaluation_generation.hash(&mut hasher);
        self.selection.hash(&mut hasher);
        self.settings.show_ghosts.hash(&mut hasher);
        let key = hasher.finish();
        if key == self.renderable_key {
            return;
        }
        self.renderable_key = key;

        let mut wanted: Vec<NodeId> = Vec::new();
        for id in self.top_level_selection() {
            wanted.extend(std::iter::once(id).chain(self.scene.descendants(id)));
        }
        if self.settings.show_ghosts {
            // Hidden nodes, so a subtracted tool body can be seen while it is
            // being positioned (spec section 6.1).
            for id in self.scene.depth_first() {
                if !self.scene.node(id).visible {
                    wanted.extend(std::iter::once(id).chain(self.scene.descendants(id)));
                }
            }
        }
        wanted.sort_unstable();
        wanted.dedup();
        wanted.retain(|id| self.evaluated.node_meshes.contains_key(id));

        let mut fresh = std::collections::BTreeMap::new();
        for id in wanted {
            let mesh = self.evaluated.node_meshes[&id].clone();
            fresh.insert(id, Renderable::prepare(&mesh));
        }
        self.node_renderables = fresh;
        self.invalidate_image();
    }

    /// Run whatever command the pressed keys are bound to. Text fields keep the
    /// keyboard when they have focus, so typing a dimension never fires a
    /// shortcut.
    pub(crate) fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if self.modal != Modal::None || self.recording.is_some() || self.rename.is_some() {
            return;
        }
        if ctx.wants_keyboard_input() {
            return;
        }
        let events: Vec<(egui::Key, egui::Modifiers)> = ctx.input(|input| {
            input
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Key { key, pressed: true, modifiers, .. } => Some((*key, *modifiers)),
                    _ => None,
                })
                .collect()
        });
        for (key, modifiers) in events {
            let chord = ui::chord_from_egui(key, modifiers);
            if let Some(command) = self.keymap.command_for(&chord) {
                self.run(command);
            }
        }
    }

    pub(crate) fn menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                self.file_menu(ui);
                self.edit_menu(ui);
                self.add_menu(ui);
                self.view_menu(ui);
                self.manipulate_menu(ui);
                self.help_menu(ui);
                ui.separator();
                self.mode_toolbar(ui);
            });
        });
    }

    fn command_item(&mut self, ui: &mut egui::Ui, command: Command, enabled: bool) {
        let label = ui::menu_label(&self.keymap, command);
        if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
            self.run(command);
            ui.close();
        }
    }

    fn file_menu(&mut self, ui: &mut egui::Ui) {
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

            ui.separator();
            self.command_item(ui, Command::Save, true);
            self.command_item(ui, Command::SaveAs, true);
            ui.separator();
            self.command_item(ui, Command::Export, true);
            ui.separator();
            if ui.button("Scene settings...").clicked() {
                self.modal = Modal::SceneSettings;
                ui.close();
            }
            ui.separator();
            self.command_item(ui, Command::Quit, true);
        });
    }

    fn edit_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Edit", |ui| {
            let undo = self.history.undo_label().map(|l| format!("Undo {l}"));
            let redo = self.history.redo_label().map(|l| format!("Redo {l}"));
            let can_undo = self.history.can_undo();
            let can_redo = self.history.can_redo();
            let undo_text =
                format!("{}\t{}", undo.unwrap_or_else(|| "Undo".into()), self.keymap.shortcut_text(Command::Undo));
            let redo_text =
                format!("{}\t{}", redo.unwrap_or_else(|| "Redo".into()), self.keymap.shortcut_text(Command::Redo));
            if ui.add_enabled(can_undo, egui::Button::new(undo_text)).clicked() {
                self.run(Command::Undo);
                ui.close();
            }
            if ui.add_enabled(can_redo, egui::Button::new(redo_text)).clicked() {
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
            self.command_item(ui, Command::Group, has_selection);
            self.command_item(ui, Command::Rename, has_selection);
            self.command_item(ui, Command::ToggleVisibility, has_selection);
            self.command_item(ui, Command::MoveUp, has_selection);
            self.command_item(ui, Command::MoveDown, has_selection);
            ui.separator();
            if ui.button("Keyboard and mouse...").clicked() {
                self.modal = Modal::Keymap;
                ui.close();
            }
        });
    }

    /// Built entirely from the primitive registry: adding a primitive type puts
    /// it in this menu with no code here to change (spec section 3.2).
    fn add_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Add", |ui| {
            for op in GroupOp::ALL {
                if ui.button(format!("{} group", op.label())).clicked() {
                    self.add_node(None, op);
                    ui.close();
                }
            }
            ui.separator();
            for category in primitive::categories() {
                ui.menu_button(category, |ui| {
                    for spec in primitive::REGISTRY.iter().filter(|s| s.category == category) {
                        if ui.button(spec.label).clicked() {
                            self.add_node(Some(spec.type_id), GroupOp::Union);
                            ui.close();
                        }
                    }
                });
            }
        });
    }

    fn view_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("View", |ui| {
            self.command_item(ui, Command::FrameSelection, !self.selection.is_empty());
            self.command_item(ui, Command::FrameAll, true);
            ui.separator();
            for command in [
                Command::ViewTop,
                Command::ViewBottom,
                Command::ViewFront,
                Command::ViewBack,
                Command::ViewLeft,
                Command::ViewRight,
                Command::ViewIsometric,
            ] {
                self.command_item(ui, command, true);
            }
            ui.separator();
            let ortho = self.scene.camera.orthographic;
            let label = format!(
                "{}\t{}",
                if ortho { "Switch to perspective" } else { "Switch to orthographic" },
                self.keymap.shortcut_text(Command::ToggleProjection)
            );
            if ui.button(label).clicked() {
                self.run(Command::ToggleProjection);
                ui.close();
            }
            ui.separator();
            for (mode, command) in [
                (DisplayMode::Shaded, Command::DisplayShaded),
                (DisplayMode::ShadedWithEdges, Command::DisplayShadedEdges),
                (DisplayMode::Wireframe, Command::DisplayWireframe),
            ] {
                let selected = self.settings.display_mode == mode;
                let label = format!(
                    "{} {}\t{}",
                    if selected { "*" } else { " " },
                    mode.label(),
                    self.keymap.shortcut_text(command)
                );
                if ui.button(label).clicked() {
                    self.run(command);
                    ui.close();
                }
            }
            ui.separator();
            for (on, command, label) in [
                (self.scene.settings.grid_visible, Command::ToggleGrid, "Ground grid"),
                (self.settings.show_bounding_box, Command::ToggleBoundingBox, "Bounding box"),
                (self.settings.show_ghosts, Command::ToggleGhosts, "Hidden nodes as ghosts"),
            ] {
                let text = format!("{} {label}\t{}", if on { "*" } else { " " }, self.keymap.shortcut_text(command));
                if ui.button(text).clicked() {
                    self.run(command);
                    ui.close();
                }
            }
        });
    }

    fn manipulate_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Manipulate", |ui| {
            for (mode, command) in [
                (Mode::Move, Command::ModeMove),
                (Mode::Rotate, Command::ModeRotate),
                (Mode::Resize, Command::ModeResize),
            ] {
                let text = format!(
                    "{} {}\t{}",
                    if self.mode == mode { "*" } else { " " },
                    mode.label(),
                    self.keymap.shortcut_text(command)
                );
                if ui.button(text).clicked() {
                    self.run(command);
                    ui.close();
                }
            }
            ui.separator();
            let text = format!(
                "Handle frame: {}\t{}",
                self.settings.handle_frame.label(),
                self.keymap.shortcut_text(Command::ToggleHandleFrame)
            );
            if ui.button(text).clicked() {
                self.run(Command::ToggleHandleFrame);
                ui.close();
            }
            ui.separator();
            ui.label("Hold Alt to drag freely, Shift to snap coarsely,");
            ui.label("Ctrl to resize about the centre or keep proportions.");
        });
    }

    fn help_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Help", |ui| {
            if ui.button("About ScadStudio").clicked() {
                self.modal = Modal::About;
                ui.close();
            }
        });
    }

    fn mode_toolbar(&mut self, ui: &mut egui::Ui) {
        for mode in Mode::ALL {
            let command = match mode {
                Mode::Move => Command::ModeMove,
                Mode::Rotate => Command::ModeRotate,
                Mode::Resize => Command::ModeResize,
            };
            let selected = self.mode == mode;
            let response = ui.selectable_label(selected, mode.label()).on_hover_text(format!(
                "{} ({})",
                mode.label(),
                self.keymap.shortcut_text(command)
            ));
            if response.clicked() {
                self.run(command);
            }
        }
        ui.separator();
        if ui
            .selectable_label(self.settings.handle_frame == HandleFrame::World, "World frame")
            .on_hover_text(format!(
                "Handles work in the {} frame ({})",
                self.settings.handle_frame.label(),
                self.keymap.shortcut_text(Command::ToggleHandleFrame)
            ))
            .clicked()
        {
            self.run(Command::ToggleHandleFrame);
        }
    }

    pub(crate) fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Display unit, so what the numbers mean is never in doubt.
                egui::ComboBox::from_id_salt("status-unit").selected_text(self.unit().suffix()).width(56.0).show_ui(
                    ui,
                    |ui| {
                        for unit in Unit::ALL {
                            // Switching never rescales the model: the unit only
                            // changes what the fields read (spec section 4).
                            if ui.selectable_label(self.unit() == unit, unit.suffix()).clicked() {
                                self.scene.settings.unit = unit;
                                self.fields.clear();
                            }
                        }
                    },
                );
                ui.separator();

                ui.label("Segments");
                let mut segments = self.scene.settings.default_segments as f64;
                let response =
                    ui.add(egui::DragValue::new(&mut segments).range(3.0..=512.0).speed(0.5).max_decimals(0));
                if response.changed() {
                    self.edit("Default segments", Some("scene:segments"));
                    self.scene.settings.default_segments = segments.round() as u32;
                }
                response.on_hover_text(
                    "Segments for curved surfaces. Vertices sit on the circumscribed circle, \
                     so a cylinder of diameter 50 still measures 50 at its widest.",
                );
                ui.separator();

                if ui.button("Export...").clicked() {
                    self.run(Command::Export);
                }
                ui.separator();

                // The message area, and progress for whatever is in flight.
                if let Some(job) = &self.export_job {
                    let fraction = job.fraction();
                    ui.add(egui::ProgressBar::new(fraction).desired_width(120.0).show_percentage());
                    ui.label(format!(
                        "Exporting {} ({}s of {}s allowed)",
                        job.format_label,
                        job.elapsed().as_secs(),
                        job.limit().as_secs()
                    ));
                    if ui.button("Cancel").clicked() {
                        job.cancel();
                    }
                } else if self.worker.is_busy() {
                    ui.add(egui::Spinner::new().size(14.0));
                    ui.label("Evaluating...");
                } else {
                    let colour = match &self.status {
                        Status::Warning(_) => Some(ui.visuals().warn_fg_color),
                        _ => None,
                    };
                    let text = egui::RichText::new(self.status_text());
                    ui.label(match colour {
                        Some(colour) => text.color(colour),
                        None => text,
                    });
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(ui::describe_counts(self.scene.len(), self.evaluated.mesh.triangle_count()));
                    if let Some(elapsed) = self.worker.last_elapsed {
                        ui.separator();
                        ui.label(ui::describe_elapsed(elapsed));
                    }
                    if !self.evaluated.errors.is_empty() {
                        ui.separator();
                        let names: Vec<&str> = self.evaluated.errors.iter().map(|e| e.name.as_str()).collect();
                        ui.colored_label(ui.visuals().error_fg_color, format!("Failed: {}", names.join(", ")))
                            .on_hover_text(
                                self.evaluated
                                    .errors
                                    .iter()
                                    .map(|e| format!("{}: {}", e.name, e.message))
                                    .collect::<Vec<_>>()
                                    .join("\n"),
                            );
                    }
                });
            });
        });
    }

    pub(crate) fn modals(&mut self, ctx: &egui::Context) {
        match self.modal {
            Modal::None => {}
            Modal::Export => self.export_window(ctx),
            Modal::Keymap => self.keymap_window(ctx),
            Modal::SceneSettings => self.scene_settings_window(ctx),
            Modal::About => self.about_window(ctx),
            Modal::Error => self.error_window(ctx),
            Modal::ConfirmQuit => self.confirm_quit_window(ctx),
        }
    }

    fn export_window(&mut self, ctx: &egui::Context) {
        let mut open = true;
        egui::Window::new("Export")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                egui::Grid::new("export-grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                    ui.label("Format");
                    egui::ComboBox::from_id_salt("export-format").selected_text(self.export_format.label()).show_ui(
                        ui,
                        |ui| {
                            for format in Format::ALL {
                                ui.selectable_value(&mut self.export_format, format, format.label());
                            }
                        },
                    );
                    ui.end_row();

                    ui.label("Units");
                    if self.export_format.carries_units() {
                        ui.label("Millimetres, recorded in the file");
                    } else {
                        // For formats that do not carry units, state the
                        // assumption (spec section 9).
                        ui.label(
                            egui::RichText::new(
                                "This format does not record units. Numbers are written in millimetres.",
                            )
                            .weak(),
                        );
                    }
                    ui.end_row();

                    ui.label("Scale");
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.export_scale).desired_width(70.0));
                        ui.label(match scadstudio_core::unit::parse_number(&self.export_scale) {
                            Some(v) if v > 0.0 => format!("x{}", format_number(v, 4)),
                            _ => "must be a positive number".to_string(),
                        });
                    });
                    ui.end_row();

                    ui.label("Contents");
                    ui.vertical(|ui| {
                        ui.radio_value(&mut self.export_selection_only, false, "The whole scene");
                        ui.add_enabled_ui(!self.selection.is_empty(), |ui| {
                            ui.radio_value(&mut self.export_selection_only, true, "The current selection");
                        });
                    });
                    ui.end_row();
                });

                ui.separator();
                if self.evaluated.errors.is_empty() {
                    ui.label(format!(
                        "{} triangles will be verified as watertight before anything is written.",
                        self.evaluated.mesh.triangle_count()
                    ));
                } else {
                    ui.colored_label(
                        ui.visuals().error_fg_color,
                        "The scene has geometry that could not be evaluated; export will refuse.",
                    );
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Export...").clicked() {
                        self.start_export();
                    }
                    if ui.button("Cancel").clicked() {
                        self.modal = Modal::None;
                    }
                });
            });
        if !open {
            self.modal = Modal::None;
        }
    }

    fn scene_settings_window(&mut self, ctx: &egui::Context) {
        let mut open = true;
        egui::Window::new("Scene settings")
            .open(&mut open)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                egui::Grid::new("scene-settings").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                    ui.label("Display unit");
                    egui::ComboBox::from_id_salt("settings-unit").selected_text(self.unit().suffix()).show_ui(
                        ui,
                        |ui| {
                            for unit in Unit::ALL {
                                if ui.selectable_label(self.unit() == unit, unit.suffix()).clicked() {
                                    self.scene.settings.unit = unit;
                                    self.fields.clear();
                                }
                            }
                        },
                    );
                    ui.end_row();

                    ui.label("Default segments");
                    let mut segments = self.scene.settings.default_segments as f64;
                    if ui.add(egui::DragValue::new(&mut segments).range(3.0..=512.0).max_decimals(0)).changed() {
                        self.edit("Default segments", Some("scene:segments"));
                        self.scene.settings.default_segments = segments.round() as u32;
                    }
                    ui.end_row();

                    ui.label("Grid spacing");
                    let unit = self.unit();
                    let mut spacing = unit.from_mm(self.scene.settings.grid_spacing);
                    if ui
                        .add(egui::DragValue::new(&mut spacing).range(1e-6..=1e6).speed(0.1))
                        .on_hover_text("Also the snap increment for move and resize drags.")
                        .changed()
                    {
                        self.edit("Grid spacing", Some("scene:grid"));
                        self.scene.settings.grid_spacing = unit.to_mm(spacing).max(1e-6);
                    }
                    ui.label(unit.suffix());
                    ui.end_row();

                    ui.label("Rotation snap");
                    ui.add(egui::DragValue::new(&mut self.settings.rotate_snap_deg).range(0.1..=90.0).suffix(" deg"));
                    ui.end_row();

                    ui.label("Show grid");
                    ui.checkbox(&mut self.scene.settings.grid_visible, "");
                    ui.end_row();
                });
                ui.separator();
                ui.label("Notes");
                let mut notes = self.scene.settings.notes.clone();
                if ui.add(egui::TextEdit::multiline(&mut notes).desired_rows(4).desired_width(360.0)).changed() {
                    self.edit("Notes", Some("scene:notes"));
                    self.scene.settings.notes = notes;
                }
                ui.separator();
                if ui.button("Close").clicked() {
                    self.modal = Modal::None;
                }
            });
        if !open {
            self.modal = Modal::None;
        }
    }

    /// The keymap editor: every command grouped by area, with a search box, the
    /// current binding shown, and click-to-record (spec section 8.2).
    fn keymap_window(&mut self, ctx: &egui::Context) {
        // Recording swallows the next key press, so it cannot also fire the
        // command it is being bound to.
        if let Some(command) = self.recording {
            let pressed: Option<(egui::Key, egui::Modifiers)> = ctx.input(|input| {
                input.events.iter().find_map(|event| match event {
                    egui::Event::Key { key, pressed: true, modifiers, .. } => Some((*key, *modifiers)),
                    _ => None,
                })
            });
            if let Some((key, modifiers)) = pressed {
                if key == egui::Key::Escape {
                    self.recording = None;
                } else {
                    let chord = ui::chord_from_egui(key, modifiers);
                    match self.keymap.set(command, chord.clone(), false) {
                        Ok(()) => {
                            self.recording = None;
                            let _ = config::save_keymap(&self.keymap);
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
        }

        let mut open = true;
        egui::Window::new("Keyboard and mouse").open(&mut open).default_width(560.0).default_height(560.0).show(
            ctx,
            |ui| {
                ui.horizontal(|ui| {
                    ui.label("Preset");
                    let mut preset = self.keymap.preset;
                    egui::ComboBox::from_id_salt("keymap-preset").selected_text(preset.label()).show_ui(ui, |ui| {
                        for option in Preset::ALL {
                            ui.selectable_value(&mut preset, option, option.label());
                        }
                    });
                    if preset != self.keymap.preset {
                        // A preset is a starting point the user can then modify.
                        self.keymap.switch_preset(preset);
                        let _ = config::save_keymap(&self.keymap);
                        self.status = Status::Info(format!("Keymap preset: {}", preset.label()));
                    }
                    if ui.button("Reset everything to the preset").clicked() {
                        self.keymap.reset_all();
                        let _ = config::save_keymap(&self.keymap);
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Export...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("ScadStudio keymap", &["json"])
                            .set_file_name("scadstudio-keymap.json")
                            .save_file()
                        {
                            if let Err(e) = std::fs::write(&path, self.keymap.to_text()) {
                                self.fail("Could not write the keymap", &e.to_string());
                            }
                        }
                    }
                    if ui.button("Import...").clicked() {
                        if let Some(path) =
                            rfd::FileDialog::new().add_filter("ScadStudio keymap", &["json"]).pick_file()
                        {
                            match std::fs::read_to_string(&path)
                                .map_err(|e| e.to_string())
                                .and_then(|t| Keymap::from_text(&t))
                            {
                                Ok(keymap) => {
                                    self.keymap = keymap;
                                    let _ = config::save_keymap(&self.keymap);
                                }
                                Err(e) => self.fail("Could not read the keymap", &e),
                            }
                        }
                    }
                });
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Navigation");
                });
                egui::Grid::new("nav-grid").num_columns(3).spacing([10.0, 6.0]).show(ui, |ui| {
                    let mut nav = self.keymap.nav;
                    for (label, drag) in [("Orbit", 0), ("Pan", 1)] {
                        ui.label(label);
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
                    ui.label("Zoom wheel");
                    ui.checkbox(&mut nav.invert_zoom, "Inverted");
                    ui.label("");
                    ui.end_row();
                    if nav != self.keymap.nav {
                        // Applies immediately, without a restart.
                        self.keymap.nav = nav;
                        let _ = config::save_keymap(&self.keymap);
                    }
                });
                if self.keymap.nav.orbit.button == self.keymap.nav.pan.button
                    && self.keymap.nav.orbit.ctrl == self.keymap.nav.pan.ctrl
                    && self.keymap.nav.orbit.shift == self.keymap.nav.pan.shift
                    && self.keymap.nav.orbit.alt == self.keymap.nav.pan.alt
                {
                    ui.colored_label(
                        ui.visuals().warn_fg_color,
                        "Orbit and pan are on the same binding; pan will never fire.",
                    );
                }

                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Search");
                    ui.add(egui::TextEdit::singleline(&mut self.keymap_search).desired_width(200.0));
                    if ui.button("Clear").clicked() {
                        self.keymap_search.clear();
                    }
                });

                let needle = self.keymap_search.to_lowercase();
                egui::ScrollArea::vertical().show(ui, |ui| {
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
                        ui.add_space(4.0);
                        ui.strong(area.label());
                        egui::Grid::new(format!("keymap-{}", area.label())).num_columns(3).spacing([10.0, 4.0]).show(
                            ui,
                            |ui| {
                                for command in commands {
                                    ui.label(command.label());
                                    let recording = self.recording == Some(command);
                                    let text = if recording {
                                        "press a key...".to_string()
                                    } else {
                                        let shown = self.keymap.shortcut_text(command);
                                        if shown.is_empty() {
                                            "unbound".to_string()
                                        } else {
                                            shown
                                        }
                                    };
                                    if ui.add(egui::Button::new(text).min_size(egui::vec2(130.0, 0.0))).clicked() {
                                        self.recording = Some(command);
                                    }
                                    if ui.small_button("Reset").clicked() {
                                        self.keymap.reset(command);
                                        let _ = config::save_keymap(&self.keymap);
                                    }
                                    ui.end_row();
                                }
                            },
                        );
                    }
                });
            },
        );
        if !open {
            self.modal = Modal::None;
            self.recording = None;
        }

        if let Some((command, chord, holder)) = self.keymap_conflict.clone() {
            egui::Window::new("That combination is already in use")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 40.0))
                .show(ctx, |ui| {
                    ui.label(format!("{chord} is currently bound to \"{}\".", holder.label()));
                    ui.label(format!("Reassign it to \"{}\"?", command.label()));
                    ui.horizontal(|ui| {
                        if ui.button("Reassign").clicked() {
                            let _ = self.keymap.set(command, chord.clone(), true);
                            let _ = config::save_keymap(&self.keymap);
                            self.keymap_conflict = None;
                        }
                        if ui.button("Cancel").clicked() {
                            self.keymap_conflict = None;
                        }
                    });
                });
        }
    }

    fn about_window(&mut self, ctx: &egui::Context) {
        let mut open = true;
        egui::Window::new(format!("About {APP_NAME}"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.heading(APP_NAME);
                ui.label(format!("Version {VERSION}"));
                ui.add_space(6.0);
                ui.label("Parametric 3D modelling with exact metric dimensions.");
                ui.label("Everything is stored in millimetres; the display unit only changes what you read.");
                ui.add_space(6.0);
                ui.label(format!("Project files: .{PROJECT_EXTENSION}"));
                ui.label(format!("Settings: {}", config::config_dir().display()));
                if config::portable_mode() {
                    ui.label("Running in portable mode: settings live beside the executable.");
                }
                ui.add_space(6.0);
                if ui.button("Close").clicked() {
                    self.modal = Modal::None;
                }
            });
        if !open {
            self.modal = Modal::None;
        }
    }

    /// Failures are shown in a scrollable, copyable window with the specific
    /// reason, never a generic message (spec section 9).
    fn error_window(&mut self, ctx: &egui::Context) {
        let mut open = true;
        let title = self.error_title.clone();
        let mut detail = self.error_detail.clone();
        egui::Window::new(&title)
            .open(&mut open)
            .collapsible(false)
            .default_width(520.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    // A read-only multiline field, so the text can be selected
                    // and copied.
                    ui.add(
                        egui::TextEdit::multiline(&mut detail)
                            .desired_width(f32::INFINITY)
                            .desired_rows(6)
                            .interactive(true),
                    );
                });
                ui.horizontal(|ui| {
                    if ui.button("Copy").clicked() {
                        ui.ctx().copy_text(self.error_detail.clone());
                    }
                    if ui.button("Close").clicked() {
                        self.modal = Modal::None;
                    }
                });
            });
        if !open {
            self.modal = Modal::None;
        }
    }

    fn confirm_quit_window(&mut self, ctx: &egui::Context) {
        egui::Window::new("Unsaved changes")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label("This project has changes that have not been saved.");
                ui.horizontal(|ui| {
                    if ui.button("Save and quit").clicked() {
                        self.save();
                        if !self.unsaved() {
                            self.confirm_quit();
                        }
                        self.modal = Modal::None;
                    }
                    if ui.button("Quit without saving").clicked() {
                        self.confirm_quit();
                        self.modal = Modal::None;
                    }
                    if ui.button("Cancel").clicked() {
                        self.modal = Modal::None;
                    }
                });
            });
    }
}
