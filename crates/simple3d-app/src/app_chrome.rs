//! The window furniture: keyboard dispatch, menu bar, status bar and the modal
//! windows (export, keymap editor, scene settings, about, errors, quit
//! confirmation).

use crate::app::{App, Modal, Status, APP_NAME, PROJECT_EXTENSION, VERSION};
use crate::gizmo::Mode;
use crate::render::Renderable;
use crate::theme;
use crate::ui;
use crate::window_chrome;
use simple3d_core::config::{self, DisplayMode, Panel, Side};
use simple3d_core::keymap::{Area, Command, Keymap, MouseButton, Preset};
use simple3d_core::primitive;
use simple3d_core::scene::{GroupOp, NodeId};
use simple3d_core::unit::{format_number, Unit};
use simple3d_export::Format;
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

    /// The menu bar, which is also the window's title bar.
    ///
    /// One bar rather than two: the decorations are off (see `window_chrome`),
    /// and a separate strip holding nothing but three buttons would waste a row
    /// of a window that is mostly viewport. Files and every browser merge them
    /// the same way.
    pub(crate) fn menu_bar(&mut self, ctx: &egui::Context) {
        // The top two corners of the window are this bar's corners.
        let radius = if window_chrome::is_maximized(ctx) { 0 } else { window_chrome::corner_radius() };
        let frame = egui::Frame::NONE
            .fill(theme::token::SURFACE_2)
            .corner_radius(egui::CornerRadius { nw: radius, ne: radius, sw: 0, se: 0 })
            .inner_margin(egui::Margin {
                left: 8,
                right: if window_chrome::CHROME == window_chrome::Chrome::Windows { 0 } else { 6 },
                top: 0,
                bottom: 0,
            });
        let maximized = window_chrome::is_maximized(ctx);
        let panel =
            egui::TopBottomPanel::top("menu").frame(frame).exact_height(window_chrome::bar_height()).show(ctx, |ui| {
                // Claimed before the contents are laid out, so that every menu
                // and button placed after it sits on top and takes its own
                // press: what is left over is title bar, and drags the window.
                let bar = ui.max_rect();
                let drag = ui.interact(bar, egui::Id::new("title-bar"), egui::Sense::click_and_drag());
                window_chrome::header_underline(ui, bar);
                ui.horizontal_centered(|ui| {
                    // The application's own name, once, at the left.
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(APP_NAME).size(13.0).strong().color(theme::token::TEXT_HI),
                        )
                        .selectable(false),
                    );
                    ui.add_space(10.0);
                    egui::MenuBar::new().ui(ui, |ui| {
                        self.file_menu(ui);
                        self.edit_menu(ui);
                        self.add_menu(ui);
                        self.view_menu(ui);
                        self.manipulate_menu(ui);
                        self.help_menu(ui);
                    });
                    // The window buttons at the corner, then the document name
                    // inside them: what is open and whether it still matches
                    // what is on disk.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        window_chrome::window_buttons(ui, maximized);
                        ui.add_space(8.0);
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
                drag
            });
        window_chrome::title_bar_drag(ctx, &panel.inner, maximized);
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
                (self.scene.settings.axes_visible[0], Command::ToggleAxisX, "X axis"),
                (self.scene.settings.axes_visible[1], Command::ToggleAxisY, "Y axis"),
                (self.scene.settings.axes_visible[2], Command::ToggleAxisZ, "Z axis"),
                (self.settings.show_bounding_box, Command::ToggleBoundingBox, "Bounding box"),
                (self.settings.show_ghosts, Command::ToggleGhosts, "Hidden nodes as ghosts"),
                (!self.settings.layout.docks_hidden, Command::ToggleDocks, "Side docks"),
            ] {
                let text = format!("{} {label}\t{}", if on { "*" } else { " " }, self.keymap.shortcut_text(command));
                if ui.button(text).clicked() {
                    self.run(command);
                    ui.close();
                }
            }
            ui.separator();
            // Where the panels are is a view decision, so it lives here with the
            // rest of them rather than in a preferences window.
            ui.menu_button("Panels", |ui| {
                for panel in Panel::ALL {
                    let side = self.settings.layout.side_of(panel);
                    let collapsed = self.settings.layout.is_collapsed(panel);
                    ui.menu_button(panel.label(), |ui| {
                        for option in Side::ALL {
                            let text = format!("{} {}", if side == option { "*" } else { " " }, option.label());
                            if ui.button(text).clicked() {
                                let index = self.settings.layout.panels(option).len();
                                self.settings.layout.move_to(panel, option, index);
                                ui.close();
                            }
                        }
                        ui.separator();
                        if ui.button(if collapsed { "* Rolled up" } else { "  Rolled up" }).clicked() {
                            self.settings.layout.toggle_collapsed(panel);
                            ui.close();
                        }
                    });
                }
            });
            self.command_item(ui, Command::ResetLayout, true);
            ui.separator();
            let text = format!("{} Reduce motion", if self.settings.reduce_motion { "*" } else { " " });
            if ui.button(text).on_hover_text("Turn the camera to a new view instantly, with no transition").clicked() {
                self.settings.reduce_motion = !self.settings.reduce_motion;
                ui.close();
            }
        });
    }

    fn manipulate_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Manipulate", |ui| {
            // Driven from `Mode::ALL`, so a tool the manipulator gains cannot
            // be missing from the menu.
            for mode in Mode::ALL {
                let command = crate::panel_toolrail::tool(mode).1;
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
            if ui.button("About Simple 3D").clicked() {
                self.modal = Modal::About;
                ui.close();
            }
        });
    }

    pub(crate) fn status_bar(&mut self, ctx: &egui::Context) {
        // And the bottom two are this one's.
        let radius = if window_chrome::is_maximized(ctx) { 0 } else { window_chrome::corner_radius() };
        let frame = egui::Frame::NONE
            .fill(theme::token::SURFACE_2)
            .corner_radius(egui::CornerRadius { nw: 0, ne: 0, sw: radius, se: radius })
            .inner_margin(egui::Margin { left: 8, right: 8, top: 0, bottom: 0 });
        egui::TopBottomPanel::bottom("status").frame(frame).exact_height(theme::metric::STATUS_BAR).show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);

                // Left to right: what is selected, how big it is, what the
                // numbers snap to, and what unit they are in.
                ui.add(egui::Label::new(theme::value(self.selection_summary())).selectable(false));
                dot(ui);
                ui.add(egui::Label::new(theme::numeric(self.selection_size_text())).selectable(false));
                dot(ui);
                // The step and the grid are two different numbers, and both are
                // on the bar: "why did it jump 10" is answered here.
                let unit = self.unit();
                let step = simple3d_core::unit::format_length(self.move_snap(), unit);
                let grid = simple3d_core::unit::format_length(self.scene.settings.grid_spacing, unit);
                ui.add(egui::Label::new(theme::numeric(format!("Step {step} {}", unit.suffix()))).selectable(false))
                    .on_hover_text("How far one nudge, and one snapped step of a drag, goes. Set it in Transform.");
                dot(ui);
                ui.add(egui::Label::new(theme::numeric(format!("Grid {grid} {}", unit.suffix()))).selectable(false))
                    .on_hover_text("Ground grid spacing; set it in the Document panel");
                dot(ui);

                // The unit is a click, not a trip to a settings window: it is
                // the one piece of document state read on every single field.
                let unit = self.unit();
                egui::ComboBox::from_id_salt("status-unit")
                    .selected_text(theme::numeric(unit.suffix()))
                    .width(52.0)
                    .show_ui(ui, |ui| {
                        for option in Unit::ALL {
                            // Switching never rescales the model: the unit only
                            // changes what the fields read (spec section 4).
                            if ui.selectable_label(unit == option, option.suffix()).clicked() {
                                self.scene.settings.unit = option;
                                self.fields.clear();
                            }
                        }
                    });

                dot(ui);

                // The message area, and progress for whatever is in flight.
                if let Some(job) = &self.export_job {
                    let fraction = job.fraction();
                    ui.add(egui::ProgressBar::new(fraction).desired_width(110.0).show_percentage());
                    ui.add(
                        egui::Label::new(theme::value(format!(
                            "Exporting {} ({}s of {}s allowed)",
                            job.format_label,
                            job.elapsed().as_secs(),
                            job.limit().as_secs()
                        )))
                        .selectable(false),
                    );
                    if ui.small_button("Cancel").clicked() {
                        job.cancel();
                    }
                } else if self.worker.is_busy() {
                    ui.add(egui::Spinner::new().size(12.0));
                    ui.add(egui::Label::new(theme::value("Evaluating\u{2026}")).selectable(false));
                } else {
                    let colour = match &self.status {
                        Status::Warning(_) => theme::token::ACCENT,
                        _ => theme::token::TEXT_LO,
                    };
                    // A message fades out once it has had time to be read, so
                    // the bar stops reporting something that finished minutes
                    // ago as though it had just happened.
                    let opacity = crate::app::status_opacity(&self.status, self.status_at.elapsed());
                    if opacity > 0.0 {
                        // The message is the one thing on this bar whose length
                        // is not ours to choose: a message that names a file
                        // names its whole path. It gets what is left once the
                        // readout at the right end has had its room, and is
                        // elided into that -- running underneath the readout,
                        // which is what an unbounded label does, leaves both
                        // unreadable.
                        let text = self.status_text();
                        let room = (ui.available_width() - theme::metric::STATUS_READOUT).max(0.0);
                        ui.scope(|ui| {
                            ui.set_max_width(room);
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(&text)
                                        .size(theme::font::LABEL)
                                        .color(colour.gamma_multiply(opacity)),
                                )
                                .truncate()
                                .selectable(false),
                            )
                            .on_hover_text(&text);
                        });
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(
                        egui::Label::new(theme::numeric(ui::describe_counts(
                            self.scene.len(),
                            self.evaluated.mesh.triangle_count(),
                        )))
                        .selectable(false),
                    );
                    if let Some(elapsed) = self.worker.last_elapsed {
                        dot(ui);
                        ui.add(egui::Label::new(theme::numeric(ui::describe_elapsed(elapsed))).selectable(false));
                    }
                    if !self.evaluated.errors.is_empty() {
                        dot(ui);
                        let names: Vec<&str> = self.evaluated.errors.iter().map(|e| e.name.as_str()).collect();
                        ui.colored_label(theme::token::DANGER, format!("Failed: {}", names.join(", "))).on_hover_text(
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

    /// "2 selected", or the name when there is exactly one -- the name is more
    /// use than the count when the count is one.
    pub(crate) fn selection_summary(&self) -> String {
        match self.selection.len() {
            0 => "Nothing selected".to_string(),
            1 => self.scene.node(self.selection[0]).name.clone(),
            n => format!("{n} selected"),
        }
    }

    /// The bounding size of what is selected, or of the whole scene when nothing
    /// is -- the status bar's answer to "will this fit".
    pub(crate) fn selection_size_text(&self) -> String {
        let unit = self.unit();
        let bounds = match self.primary() {
            Some(id) => self.evaluated.node_world_bounds.get(&id).copied(),
            None => self.evaluated.mesh.bounds(),
        };
        match bounds {
            Some((lo, hi)) => ui::describe_size(hi - lo, unit),
            None => format!("-- {}", unit.suffix()),
        }
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
            Modal::SavePrimitive => self.save_primitive_window(ctx),
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
                        ui.label(match simple3d_core::unit::parse_number(&self.export_scale) {
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
                        .on_hover_text("How far apart the ground grid's lines are drawn.")
                        .changed()
                    {
                        self.edit("Grid spacing", Some("scene:grid"));
                        self.scene.settings.grid_spacing = unit.to_mm(spacing).max(1e-6);
                    }
                    ui.label(unit.suffix());
                    ui.end_row();

                    ui.label("Step");
                    let mut step = unit.from_mm(self.scene.settings.snap_step);
                    if ui
                        .add(egui::DragValue::new(&mut step).range(1e-6..=1e6).speed(0.05))
                        .on_hover_text("One nudge, and one snapped step of a move or resize drag.")
                        .changed()
                    {
                        self.edit("Step", Some("scene:step"));
                        self.scene.settings.snap_step = unit.to_mm(step).max(1e-6);
                    }
                    ui.label(unit.suffix());
                    ui.end_row();

                    ui.label("Rotation snap");
                    ui.add(egui::DragValue::new(&mut self.settings.rotate_snap_deg).range(0.1..=90.0).suffix(" deg"));
                    ui.end_row();

                    ui.label("Show grid");
                    ui.checkbox(&mut self.scene.settings.grid_visible, "");
                    ui.end_row();

                    ui.label("Show axes");
                    ui.horizontal(|ui| {
                        for (axis, name) in ["X", "Y", "Z"].into_iter().enumerate() {
                            ui.checkbox(&mut self.scene.settings.axes_visible[axis], name);
                        }
                    });
                    ui.end_row();

                    ui.label("Axis style");
                    ui.horizontal(|ui| {
                        for option in simple3d_core::scene::AxisStyle::ALL {
                            let showing = self.scene.settings.axis_style == option;
                            if ui.selectable_label(showing, option.label()).clicked() {
                                self.scene.settings.axis_style = option;
                            }
                        }
                    });
                    ui.end_row();

                    ui.label("Plane marks");
                    ui.checkbox(&mut self.scene.settings.plane_marks, "")
                        .on_hover_text("Mark where a principal plane cuts through a shape, on the shape itself");
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
                        self.persist_keymap();
                        self.status = Status::Info(format!("Keymap preset: {}", preset.label()));
                    }
                    if ui.button("Reset everything to the preset").clicked() {
                        self.keymap.reset_all();
                        self.persist_keymap();
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Export...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Simple 3D keymap", &["json"])
                            .set_file_name("simple3d-keymap.json")
                            .save_file()
                        {
                            if let Err(e) = std::fs::write(&path, self.keymap.to_text()) {
                                self.fail("Could not write the keymap", &e.to_string());
                            }
                        }
                    }
                    if ui.button("Import...").clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter("Simple 3D keymap", &["json"]).pick_file()
                        {
                            match std::fs::read_to_string(&path)
                                .map_err(|e| e.to_string())
                                .and_then(|t| Keymap::from_text(&t))
                            {
                                Ok(keymap) => {
                                    self.keymap = keymap;
                                    self.persist_keymap();
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
                        self.persist_keymap();
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
                                        self.persist_keymap();
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
                ui.label(format!("Settings: {}", self.config_dir().display()));
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

    /// Naming a group, or a whole project, before it goes on the palette.
    ///
    /// A window rather than an inline field because the name is going into the
    /// user's library, not into the document: it outlives this project, and it
    /// is the only thing the palette will show, so it is worth stopping to type.
    fn save_primitive_window(&mut self, ctx: &egui::Context) {
        let mut open = true;
        let mut save = false;
        egui::Window::new("Save as primitive")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                let count = self.primitive_clip.as_ref().map(|c| c.nodes.len()).unwrap_or(0);
                ui.label(format!(
                    "{count} node{} will be kept on the palette, ready to drop into any project.",
                    if count == 1 { "" } else { "s" }
                ));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label("Name");
                    let field = ui.add(
                        egui::TextEdit::singleline(&mut self.primitive_name).desired_width(240.0).hint_text("Bracket"),
                    );
                    field.request_focus();
                    if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        save = true;
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
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.add_enabled(!tidied.is_empty(), egui::Button::new("Save")).clicked() {
                        save = true;
                    }
                    if ui.button("Cancel").clicked() {
                        self.cancel_save_primitive();
                    }
                });
            });
        if save {
            self.confirm_save_primitive();
        } else if !open {
            self.cancel_save_primitive();
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

/// The status bar's separator: a dot, not a rule. A vertical line every few
/// words turns a single sentence of state into a row of boxes.
fn dot(ui: &mut egui::Ui) {
    ui.add(
        egui::Label::new(egui::RichText::new("\u{00B7}").size(theme::font::LABEL).color(theme::token::SURFACE_3))
            .selectable(false),
    );
}
