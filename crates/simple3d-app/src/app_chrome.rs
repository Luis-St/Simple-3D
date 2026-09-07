//! The window furniture: keyboard dispatch, menu bar, status bar and the modal
//! windows (export, keymap editor, about, errors, quit confirmation).

use crate::app::{App, Modal, Status, APP_NAME, PROJECT_EXTENSION, VERSION};
use crate::gizmo::Mode;
use crate::render::Renderable;
use crate::theme;
use crate::ui;
use simple3d_core::config::{self, DisplayMode, Panel, RenderEngine, Side, SnapMode};
use simple3d_core::keymap::{Area, Chord, Command, Keymap, MouseButton, Preset};
use simple3d_core::primitive;
use simple3d_core::scene::{ExportBody, GroupOp, NodeId, Scene, Visibility};
use simple3d_core::unit::{format_number, Unit};
use simple3d_export::{BodyMode, Format};

/// What one export-body mark is called, in the picker's button and in its menu.
fn body_label(body: Option<ExportBody>) -> String {
    match body {
        None => "A body of its own".to_string(),
        Some(ExportBody::Shared(key)) => format!("Body {key}"),
        Some(ExportBody::Split) => "Split into its parts".to_string(),
    }
}
use std::hash::{Hash, Hasher};

impl App {
    /// The nodes drawn as ghosts: hidden, and asked to be seen anyway. A group
    /// set to ghost carries its children with it, the way hiding it does.
    pub(crate) fn ghosts(&self) -> Vec<NodeId> {
        self.scene
            .depth_first()
            .into_iter()
            .filter(|id| self.scene.node(*id).visibility() == Visibility::Ghost)
            .collect()
    }

    /// A cheap summary of which nodes are ghosts, for the cache key: the
    /// renderables have to be rebuilt when one is turned on or off.
    fn ghost_generation(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.ghosts().hash(&mut hasher);
        hasher.finish()
    }

    /// Rebuild the per-node meshes the viewport needs -- the selection outline and
    /// the ghosts -- when either the evaluation or what is selected has changed.
    pub(crate) fn refresh_node_renderables(&mut self) {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.evaluation_generation.hash(&mut hasher);
        self.selection.hash(&mut hasher);
        self.ghost_generation().hash(&mut hasher);
        let key = hasher.finish();
        if key == self.renderable_key {
            return;
        }
        self.renderable_key = key;

        let mut fresh = std::collections::BTreeMap::new();
        // Every node the user asked to keep as a ghost, so a subtracted tool
        // body can be seen while it is being positioned (spec section 6.1). A
        // ghost group is drawn as its children, one translucent body each,
        // because that is the assembly the user is placing.
        let mut ghosts: Vec<NodeId> = Vec::new();
        for id in self.ghosts() {
            ghosts.extend(std::iter::once(id).chain(self.scene.descendants(id)));
        }
        ghosts.sort_unstable();
        ghosts.dedup();
        for id in ghosts {
            if let Some(mesh) = self.evaluated.node_meshes.get(&id) {
                fresh.insert(id, Renderable::prepare(mesh));
            }
        }
        // The selection is drawn as *what each selected node evaluates to*, and
        // not one row further down. Descending to the children outlined shapes
        // the result does not contain: a difference's cutter as two rims
        // hanging in mid-air, an intersection's whole uncut box as a cage round
        // the small lens it leaves, a pattern's source child standing where no
        // copy of it does.
        for id in self.top_level_selection() {
            if let Some(mesh) = self.evaluated.result_mesh(id) {
                fresh.insert(id, Renderable::prepare_outlined(&mesh));
            }
        }
        self.node_renderables = fresh;
        self.invalidate_image();
    }

    /// Run whatever command the pressed keys are bound to. Text fields keep the
    /// keyboard when they have focus, so typing a dimension never fires a
    /// shortcut.
    pub(crate) fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if self.modal != Modal::None || self.recording.is_some() || self.rename.is_some() {
            // The release that ends a modifier hold will be delivered somewhere
            // else, so the hold in progress is not one this window can finish.
            self.shortcut_mods.reset();
            return;
        }
        if ctx.wants_keyboard_input() {
            self.shortcut_mods.reset();
            return;
        }
        let (events, modifiers, held, pointer) = ctx.input(|input| {
            // `egui-winit` eats Ctrl+X, Ctrl+C and Ctrl+V: it pushes `Cut`,
            // `Copy` or `Paste` and returns *before* it emits the `Key` event
            // (egui-winit 0.32.3 `lib.rs:766-781`). Nothing downstream ever
            // sees the press, so those three bindings -- and anything a user
            // rebinds onto them -- could never fire. Putting the press back is
            // what keeps the keymap the single answer to what a chord does,
            // rather than hard-wiring copy and paste past it.
            let events: Vec<(egui::Key, egui::Modifiers)> = input
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Key { key, pressed: true, modifiers, .. } => Some((*key, *modifiers)),
                    egui::Event::Cut => Some((egui::Key::X, egui::Modifiers::COMMAND)),
                    egui::Event::Copy => Some((egui::Key::C, egui::Modifiers::COMMAND)),
                    egui::Event::Paste(_) => Some((egui::Key::V, egui::Modifiers::COMMAND)),
                    _ => None,
                })
                .collect();
            let pointer = input.pointer.any_down() || input.pointer.any_pressed();
            (events, input.modifiers, ui::keys_down(input), pointer)
        });
        // A press fires the longest binding everything held down satisfies, so a
        // combination of ordinary keys -- Q+W+E -- fires on the key that
        // completes it rather than every key in it firing its own binding.
        for (key, modifiers) in events {
            let pressed = key.name();
            let down = |name: &str| name == pressed || held.iter().any(|k| k == name);
            let command =
                self.keymap.command_for_press(pressed, down, modifiers.command, modifiers.shift, modifiers.alt);
            if let Some(command) = command {
                self.run(command);
            }
        }
        // A modifier held on its own, and let go of with nothing pressed under
        // it, is a binding in its own right (issue 77) -- and only that case is
        // taken here, because a chord with keys in it has already fired on the
        // press that completed it. A mouse button counts as something pressed
        // under it: Ctrl+click picks a second object, and must not also fire
        // whatever Ctrl alone is bound to.
        let completed = self.shortcut_mods.update(modifiers, held.iter().map(String::as_str), pointer);
        if let Some(chord) = completed.filter(Chord::is_modifier_only) {
            if let Some(command) = self.keymap.command_for(&chord) {
                self.run(command);
            }
        }
    }

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

    fn command_item(&mut self, ui: &mut egui::Ui, command: Command, enabled: bool) {
        let label = ui::menu_label(&self.keymap, command);
        if ui::menu_entry(ui, &label, enabled).clicked() {
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
            self.command_item(ui, Command::CloseTab, true);
            self.command_item(ui, Command::NextTab, self.tab_count() > 1);
            self.command_item(ui, Command::PreviousTab, self.tab_count() > 1);
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
            self.command_item(ui, Command::Group, has_selection);
            // Beside Group, which is the command it is a variant of. It works
            // with nothing selected too -- an empty pattern to fill later -- so
            // unlike Group it is never disabled.
            self.command_item(ui, Command::Pattern, true);
            // Issue 80 and issue 82's other half: baking a shape into the
            // triangles it evaluates to, and taking a cut result apart into the
            // pieces it is actually in.
            self.command_item(ui, Command::ConvertToMesh, !self.selection.is_empty());
            self.command_item(ui, Command::BreakApart, self.selection.len() == 1);
            // The other way to make pieces: cutting a shape that is in one
            // piece into a pattern of them (issue 82).
            self.command_item(ui, Command::SplitIntoPieces, self.selection.len() == 1);
            // Enabled only on a split, because a split is the only thing it has
            // anything to say to -- everything else was never broken apart.
            self.command_item(
                ui,
                Command::Rejoin,
                self.selection.len() == 1 && self.primary().is_some_and(|id| self.scene.node(id).is_split()),
            );
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
            // The other container a node can be (issue 67), beside the group
            // operators it belongs with. Adding one is always "an empty one to
            // fill"; wrapping the selection in a pattern is what Edit > Make a
            // pattern of the selection does.
            if ui
                .button("Pattern")
                .on_hover_text("An empty pattern, for shapes to be put into it and repeated")
                .clicked()
            {
                self.add_pattern();
                ui.close();
            }
            // The other half of the same feature: a rule the user writes
            // themselves, rather than one of the six that ship with a name
            // (issue 67). It wraps whatever is selected the way Edit > Make a
            // pattern does, and then opens the tool on it.
            if ui
                .button("Custom pattern")
                .on_hover_text("Build a repetition rule out of stages, and keep it for other projects")
                .clicked()
            {
                self.open_pattern_tool();
                ui.close();
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
                if ui::menu_entry(ui, &label, true).clicked() {
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
                (!self.settings.layout.docks_hidden, Command::ToggleDocks, "Side docks"),
            ] {
                let text = format!("{} {label}\t{}", if on { "*" } else { " " }, self.keymap.shortcut_text(command));
                if ui::menu_entry(ui, &text, true).clicked() {
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
                if ui::menu_entry(ui, &text, true).clicked() {
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
            if ui::menu_entry(ui, &text, true).clicked() {
                self.run(Command::ToggleHandleFrame);
                ui.close();
            }
            ui.separator();
            // Geometry snapping's mode (issue 68). It lived only in the document
            // settings, which the property panel shows *with nothing selected* --
            // and snapping needs something selected to have a manipulator at all,
            // so the one control and the one state were mutually exclusive and
            // the setting could not be found while doing the thing it governs.
            // It belongs here beside the handle frame, which is the same kind of
            // setting: how the manipulator behaves, not what the document holds.
            let snap_key = self.keymap.shortcut_text(Command::SnapToGeometry);
            ui.menu_button("Snap to geometry", |ui| {
                for mode in SnapMode::ALL {
                    let name = if mode == SnapMode::WhileHeld && !snap_key.is_empty() {
                        format!("{} ({snap_key})", mode.label())
                    } else {
                        mode.label().to_string()
                    };
                    let text = format!("{} {name}", if self.settings.geometry_snap == mode { "*" } else { " " });
                    if ui::menu_entry(ui, &text, true).on_hover_text(mode.description()).clicked() {
                        self.settings.geometry_snap = mode;
                        self.persist();
                        self.status = Status::Info(format!("Snap to geometry: {name}"));
                        ui.close();
                    }
                }
            });
            ui.separator();
            ui.label("Hold Alt to drag freely, Shift to snap coarsely,");
            ui.label("Ctrl to resize about the centre or keep proportions.");
            if self.settings.geometry_snap == SnapMode::WhileHeld && !snap_key.is_empty() {
                ui.label(format!("Hold {snap_key} to snap a drag onto another body."));
            }
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
        let frame = egui::Frame::NONE.fill(theme::token::SURFACE_2).inner_margin(egui::Margin {
            left: 8,
            right: 8,
            top: 0,
            bottom: 0,
        });
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

                // Which renderer draws the viewport. A dropup rather than a
                // trip to a settings window, and here beside the frame time it
                // changes: the two are read together or not at all. egui opens
                // the list upwards on its own, this near the bottom of the
                // screen.
                let engine = self.settings.render_engine;
                let mut chosen = engine;
                egui::ComboBox::from_id_salt("status-engine")
                    .selected_text(theme::value(engine.label()))
                    .width(60.0)
                    .show_ui(ui, |ui| {
                        for option in RenderEngine::ALL {
                            let entry = ui.selectable_label(engine == option, option.label());
                            if entry.on_hover_text(option.description()).clicked() {
                                chosen = option;
                            }
                        }
                    });
                if chosen != engine {
                    self.settings.render_engine = chosen;
                    // Asking again clears the last refusal, so a driver that
                    // failed once can be tried again after the user has done
                    // something about it.
                    self.gpu_error = None;
                    if chosen == RenderEngine::Cpu {
                        self.gpu = None;
                        self.gpu_texture = None;
                    }
                    // The viewport is cached on this key; the engine is not part
                    // of it, so the switch has to say the picture is stale.
                    self.image_key = u64::MAX;
                    self.persist();
                }
                if let Some(why) = &self.gpu_error {
                    if self.settings.render_engine == RenderEngine::Gpu {
                        ui.add(egui::Label::new(theme::value("\u{26a0} CPU")).selectable(false)).on_hover_text(
                            format!(
                            "The GPU renderer is not available, so the viewport is being drawn in software.\n\n{why}"
                        ),
                        );
                    }
                }

                dot(ui);

                // The message area, and progress for whatever is in flight.
                if let Some(job) = &self.split_job {
                    // Honest progress, unlike an evaluation's: the cells are
                    // counted before any of them is cut, so the bar knows how
                    // much of the job is left.
                    ui.add(egui::ProgressBar::new(job.fraction()).desired_width(110.0).show_percentage());
                    ui.add(
                        egui::Label::new(theme::value(format!(
                            "Splitting into {} cells ({}s)",
                            job.cells,
                            job.elapsed().as_secs()
                        )))
                        .selectable(false),
                    );
                    if ui
                        .small_button("Stop")
                        .on_hover_text("Abandon the split. Nothing in the document is changed.")
                        .clicked()
                    {
                        job.cancel();
                    }
                } else if let Some(job) = &self.export_job {
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
                } else if let Some(prompt) = &self.file_prompt {
                    // The dialog is a window of the desktop's, not ours, and on
                    // Linux it is the portal's -- which can be slow, or absent,
                    // or simply never answer. It waits on its own thread now, so
                    // this line is here to say what the application is waiting
                    // for rather than to apologise for being frozen.
                    ui.add(egui::Spinner::new().size(12.0));
                    ui.add(
                        egui::Label::new(theme::value(format!(
                            "{}: choosing a file\u{2026} ({}s)",
                            prompt.what(),
                            prompt.waiting_for().as_secs()
                        )))
                        .selectable(false),
                    );
                    if ui
                        .small_button("Stop waiting")
                        .on_hover_text("Give up on the file dialog. If it does answer later, the answer is ignored.")
                        .clicked()
                    {
                        self.stop_waiting_for_file();
                    }
                } else if self.worker.is_busy() {
                    // A spinner and the word "Evaluating..." was the whole of
                    // what this said, with no way out of a run that had decided
                    // to take minutes -- while an export in the same bar gets
                    // its elapsed seconds and a Cancel button. There is no
                    // honest progress to show for a boolean, which does not know
                    // how much of itself is left, but how long the user has been
                    // waiting is always knowable and Stop always available.
                    ui.add(egui::Spinner::new().size(12.0));
                    let waited = self.worker.waiting_for().unwrap_or_default();
                    let text = if waited.as_secs() >= 1 {
                        format!("Evaluating\u{2026} ({}s)", waited.as_secs())
                    } else {
                        "Evaluating\u{2026}".to_string()
                    };
                    ui.add(egui::Label::new(theme::value(text)).selectable(false));
                    if ui
                        .small_button("Stop")
                        .on_hover_text(
                            "Abandon this evaluation. The viewport keeps the last shape it managed to \
                             build, so what is on screen will be out of date until the next edit.",
                        )
                        .clicked()
                    {
                        self.worker.abandon();
                        self.status = Status::Warning("Evaluation stopped -- the viewport is out of date".into());
                    }
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
                    // The scene itself is not a node the document holds: it is
                    // the tree's root, it is always there, and counting it made
                    // an empty document report one node.
                    let nodes = self.scene.len().saturating_sub(1);
                    ui.add(
                        egui::Label::new(theme::numeric(ui::describe_counts(
                            nodes,
                            self.evaluated.mesh.triangle_count(),
                        )))
                        .selectable(false),
                    )
                    .on_hover_text("Shapes and groups in the document, and the triangles the model came out as");
                    if let Some(elapsed) = self.worker.last_elapsed {
                        dot(ui);
                        ui.add(egui::Label::new(theme::numeric(ui::describe_elapsed(elapsed))).selectable(false))
                            .on_hover_text("How long the last rebuild of the model took");
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
            // Asked of the scene rather than taken from the selection: an id the
            // scene no longer holds is a panic from `Scene::node`, and this is
            // drawn on every frame of the status bar -- so it would be the first
            // thing to run after whatever left the id behind, and would take the
            // window down before anything could prune it.
            1 => match self.scene.get(self.selection[0]) {
                Some(node) => node.name.clone(),
                None => "Nothing selected".to_string(),
            },
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
            // Nothing open, so the next dialog to open is placed afresh.
            Modal::None => self.dialog_placed = None,
            Modal::Export => self.export_window(ctx),
            Modal::Keymap => self.keymap_window(ctx),
            Modal::About => self.about_window(ctx),
            Modal::Error => self.error_window(ctx),
            Modal::ConfirmQuit => self.confirm_quit_window(ctx),
            Modal::ConfirmCloseTab => self.confirm_close_tab_window(ctx),
            Modal::SavePrimitive => self.save_primitive_window(ctx),
            Modal::PatternKind => self.pattern_kind_window(ctx),
            Modal::SplitTool => self.split_tool_window(ctx),
        }
    }

    /// One of the application's dialogs, as a window of the window system's own
    /// rather than a rectangle drawn over the viewport (issue 53).
    ///
    /// `show_viewport_immediate` opens a real top-level window and builds its
    /// contents in the same pass as the main one, so a dialog goes on reading
    /// and writing the application state directly; the deferred kind takes a
    /// `'static` callback and could not. A backend with no viewports at all --
    /// and the headless context the tests drive -- hands the body straight back
    /// as `Embedded`, and it is drawn inside the main window as it used to be.
    ///
    /// egui's `ViewportBuilder` exposes neither transient-for nor a modal flag,
    /// so the two things a dialog would otherwise inherit from its parent are
    /// asked for by hand: it opens over the middle of the parent and asks to
    /// stay above it. Wayland grants neither -- a client there does not place
    /// its own windows -- and ignores both without complaint.
    ///
    /// Every dialog is the same shape: the contents fill the window, and the
    /// buttons sit in a row of their own along the foot, right-aligned and all
    /// one size, under a rule (issue 63). The row is a panel rather than the
    /// last thing the body draws, so the buttons are in the same place in every
    /// dialog whatever the contents above them do -- and a body that scrolls can
    /// never push them off the bottom edge.
    fn dialog(
        &mut self,
        ctx: &egui::Context,
        spec: DialogSpec<'_>,
        mut body: impl FnMut(&mut Self, &mut egui::Ui),
        mut actions: impl FnMut(&mut Self, &mut egui::Ui),
    ) {
        let DialogSpec { key, title, size, resizable, fit_height, min_size } = spec;
        let id = egui::ViewportId::from_hash_of(key);
        let mut builder = egui::ViewportBuilder::default()
            .with_title(title)
            .with_icon(crate::icon::shared_icon())
            .with_inner_size(size)
            .with_resizable(resizable)
            // A dialog is not a window to put away and come back to: what is
            // fixed in size has nothing to maximise, and minimising one out of
            // sight while the application waits on it is a trap.
            .with_minimize_button(false)
            .with_maximize_button(resizable)
            .with_window_level(egui::WindowLevel::AlwaysOnTop);
        if let Some(min) = min_size {
            builder = builder.with_min_inner_size(min);
        }
        // Placed once, when the dialog opens, and never again. This body runs
        // on every frame of the parent's, and a builder that asks for a
        // position each time is a window that is put back where it started
        // whenever the parent repaints -- and, with the size asked for in the
        // same breath, one the window manager may resize under its own title
        // bar while the keyboard is somewhere else entirely.
        if self.dialog_placed != Some(id) {
            if let Some(parent) = ctx.input(|i| i.viewport().outer_rect) {
                builder = builder.with_position(parent.center() - size * 0.5);
            }
            self.dialog_placed = Some(id);
        }
        ctx.show_viewport_immediate(id, builder, |ctx, class| {
            if class == egui::ViewportClass::Embedded {
                let mut open = true;
                let mut window = egui::Window::new(title)
                    .open(&mut open)
                    .collapsible(false)
                    .resizable(resizable)
                    // Above the backdrop that blocks the main window, which is
                    // a foreground layer created just before this one. A window
                    // left at the default `Middle` would be under it and could
                    // not be clicked at all.
                    .order(egui::Order::Foreground)
                    .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO);
                // The size a dialog asks for is the size it gets here too. An
                // `egui::Window` given none sizes itself to its contents, and a
                // body that lays itself out against the room it is given has no
                // contents to be sized to: the pattern tool came out 362 px
                // wide, which is under what its two columns need, so it dropped
                // the picture and drew the numbers alone -- the preview
                // rendering nothing at all, in the one configuration
                // (`embed_dialogs`) that exists to keep the application off a
                // second window system surface.
                if resizable {
                    window = window.default_size(size);
                    if let Some(min) = min_size {
                        window = window.min_size(min);
                    }
                } else {
                    // The short forms are as tall as what is in them, which is
                    // what an auto-sized window already does and what
                    // `fit_height` asks the real window for. Only the width has
                    // to be said.
                    window = window.default_width(size.x);
                }
                window.show(ctx, |ui| {
                    if resizable {
                        // The same shape as the window of its own: the buttons
                        // are a panel along the foot, taken out of the room
                        // before the body is laid out. Stacked under a body
                        // that fills whatever it is given, they are pushed out
                        // of the window -- and the window, being sized to what
                        // is in it, grows by their height every frame until it
                        // is off the bottom of the screen.
                        egui::TopBottomPanel::bottom("dialog-actions-embedded")
                            .frame(egui::Frame::NONE.inner_margin(egui::Margin {
                                left: 0,
                                right: 0,
                                top: 8,
                                bottom: 0,
                            }))
                            .exact_height(theme::metric::DIALOG_ACTIONS)
                            .show_separator_line(true)
                            .show_inside(ui, |ui| action_row(ui, |ui| actions(self, ui)));
                        egui::CentralPanel::default().frame(egui::Frame::NONE).show_inside(ui, |ui| body(self, ui));
                    } else {
                        // A dialog whose height is its contents' has no room to
                        // take the buttons out of: they go under the body, and
                        // the window is as tall as the two together.
                        body(self, ui);
                        ui.separator();
                        action_row(ui, |ui| actions(self, ui));
                    }
                });
                if !open {
                    self.dismiss_modal();
                }
                return;
            }
            let pad = theme::metric::DIALOG_PAD;
            let footer = egui::Frame::NONE.fill(theme::token::SURFACE_1).inner_margin(egui::Margin {
                left: pad as i8,
                right: pad as i8,
                top: 8,
                bottom: 8,
            });
            egui::TopBottomPanel::bottom("dialog-actions")
                .frame(footer)
                .exact_height(theme::metric::DIALOG_ACTIONS)
                .show_separator_line(true)
                .show(ctx, |ui| action_row(ui, |ui| actions(self, ui)));
            let frame = egui::Frame::NONE.fill(theme::token::SURFACE_1).inner_margin(egui::Margin::same(pad as i8));
            let used = egui::CentralPanel::default()
                .frame(frame)
                .show(ctx, |ui| {
                    body(self, ui);
                    // What the contents actually took, measured from the top of
                    // the room inside the margin to where the next thing would
                    // go -- less the gap that would be left before it.
                    ui.cursor().top() - ui.max_rect().top() - ui.spacing().item_spacing.y
                })
                .inner;
            // A dialog whose height is its contents' asks for the height they
            // came out at, so the last line sits the same distance above the
            // rule as the first does below the window's edge -- whatever the
            // font size, the display scale, or how long the paths it prints
            // turn out to be here.
            if fit_height {
                let want = (used + pad * 2.0 + theme::metric::DIALOG_ACTIONS).ceil();
                if (want - ctx.screen_rect().height()).abs() > 1.0 {
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(size.x, want)));
                }
            }
            // The window's own close button, which no longer passes through any
            // code of ours: whatever the open dialog is, closing it cancels it.
            if ctx.input(|i| i.viewport().close_requested()) {
                self.dismiss_modal();
            }
        });
    }

    /// Close whatever dialog is open, the way that dialog is cancelled.
    fn dismiss_modal(&mut self) {
        match self.modal {
            Modal::SavePrimitive => self.cancel_save_primitive(),
            Modal::SplitTool => self.cancel_split_tool(),
            Modal::ConfirmCloseTab => self.cancel_close_tab(),
            Modal::Keymap => {
                self.recording = None;
                self.modal = Modal::None;
            }
            _ => self.modal = Modal::None,
        }
    }

    fn export_window(&mut self, ctx: &egui::Context) {
        // Tall enough for the body picker, which is the one part of this
        // window that is a list rather than a row.
        let size = if self.export_body_mode() == BodyMode::Selected {
            egui::vec2(600.0, 560.0)
        } else {
            egui::vec2(560.0, 320.0)
        };
        self.dialog(
            ctx,
            DialogSpec {
                key: "dialog-export",
                title: "Export",
                size,
                resizable: true,
                fit_height: false,
                min_size: None,
            },
            Self::export_body,
            Self::export_actions,
        );
    }

    fn export_body(&mut self, ui: &mut egui::Ui) {
        // Bottom-up: the count of what is about to be written is laid out first
        // and ends up at the foot of the contents, just over the buttons, and
        // everything else takes the room left above it. In a `bottom_up` layout
        // the items are added in the order they stack upwards, which is why the
        // summary comes first here.
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            self.export_summary_line(ui);
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| self.export_controls(ui));
        });
    }

    fn export_actions(&mut self, ui: &mut egui::Ui) {
        if ui::dialog_button(ui, "Export...", true).clicked() {
            self.start_export();
        }
        if ui::dialog_button(ui, "Cancel", true).clicked() {
            self.modal = Modal::None;
        }
    }

    fn export_controls(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("export-grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            ui.label("Format");
            egui::ComboBox::from_id_salt("export-format").selected_text(self.export_format.label()).show_ui(ui, |ui| {
                for format in Format::ALL {
                    ui.selectable_value(&mut self.export_format, format, format.label());
                }
            });
            ui.end_row();

            ui.label("Units");
            if self.export_format.carries_units() {
                ui.label("Millimetres, recorded in the file");
            } else {
                // For formats that do not carry units, state the
                // assumption (spec section 9).
                ui.label(
                    egui::RichText::new("This format does not record units. Numbers are written in millimetres.")
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

            // Only where the format has objects to keep apart; for the rest the
            // question has no answer, so it is disabled rather than ignored.
            ui.label("Bodies");
            ui.vertical(|ui| {
                let separates = self.export_format.keeps_objects_separate();
                ui.add_enabled_ui(separates, |ui| {
                    egui::ComboBox::from_id_salt("export-bodies")
                        .selected_text(self.export_body_mode().label())
                        .show_ui(ui, |ui| {
                            for mode in BodyMode::ALL {
                                ui.selectable_value(&mut self.export_bodies, mode, mode.label());
                            }
                        });
                });
                ui.label(
                    egui::RichText::new(match self.export_body_mode() {
                        _ if !separates => {
                            format!("{} holds one body; everything is merged into it.", self.export_format.label())
                        }
                        BodyMode::One => "Everything is merged into a single solid.".to_string(),
                        BodyMode::TopLevel => {
                            "One component per top-level shape or group, each named and verified on its own."
                                .to_string()
                        }
                        BodyMode::Selected => {
                            "One component per body below. What is grouped is saved with the project.".to_string()
                        }
                    })
                    .weak(),
                );
            });
            ui.end_row();
        });

        if self.export_body_mode() == BodyMode::Selected {
            ui.add_space(4.0);
            self.body_picker(ui);
        }
    }

    /// What the export is about to do. Drawn before the rest of the window and
    /// at the bottom of it, so a body list long enough to scroll cannot push the
    /// count out of sight.
    fn export_summary_line(&mut self, ui: &mut egui::Ui) {
        if self.evaluated.errors.is_empty() {
            // The count is of what will actually be written -- the scene
            // or the selection -- rather than always of the whole scene.
            let summary = self.export_summary();
            let bodies = match summary.bodies {
                1 => "one body".to_string(),
                n => format!("{n} bodies"),
            };
            ui.label(format!(
                "{} triangles in {bodies}, each verified as watertight before anything is written.",
                summary.triangles
            ));
        } else {
            ui.colored_label(
                ui.visuals().error_fg_color,
                "The scene has geometry that could not be evaluated; export will refuse.",
            );
        }
        ui.separator();
    }

    /// The rows the body picker shows: the export's own roots, and the children
    /// of every group that has been split open, in tree order.
    ///
    /// Only what can be answered is listed. A group that is a body is one solid
    /// and what is inside it is not a decision to be made -- splitting it is
    /// what turns its children into rows, which is also what makes the list
    /// short enough to read on a scene of any size.
    fn body_rows(&self) -> Vec<(NodeId, usize)> {
        fn walk(scene: &Scene, id: NodeId, depth: usize, rows: &mut Vec<(NodeId, usize)>) {
            if !scene.contains(id) {
                return;
            }
            rows.push((id, depth));
            if scene.node(id).export_body == Some(ExportBody::Split) && scene.can_split_for_export(id) {
                for &child in &scene.node(id).children {
                    walk(scene, child, depth + 1, rows);
                }
            }
        }
        let mut rows = Vec::new();
        for id in self.export_roots() {
            walk(&self.scene, id, 0, &mut rows);
        }
        rows
    }

    /// Which body each node goes in, chosen a row at a time. The marks live on
    /// the nodes, so they are saved with the project and a later export only
    /// has to say what has changed since (issue 58).
    fn body_picker(&mut self, ui: &mut egui::Ui) {
        let rows = self.body_rows();
        if rows.is_empty() {
            ui.label(theme::hint("Nothing to export, so there are no bodies to group."));
            return;
        }
        // One past the highest in use, so picking the last entry is how a new
        // body gets made.
        let offered = self.scene.highest_export_body() + 1;
        let mut change: Option<(NodeId, Option<ExportBody>)> = None;

        egui::Frame::NONE
            .fill(theme::token::SURFACE_2)
            .stroke(egui::Stroke::new(1.0_f32, theme::token::SURFACE_3))
            .inner_margin(egui::Margin::same(6))
            .show(ui, |ui| {
                // As tall as the list needs, up to whatever the window has
                // left under the controls above it; past that it scrolls.
                let room = (ui.available_height() - 24.0).max(80.0);
                let (area, restore) = theme::list_scroll_area(ui);
                area.max_height(room).auto_shrink([false, true]).show(ui, |ui| {
                    ui.set_style(restore);
                    for (id, depth) in rows {
                        let node = self.scene.node(id);
                        let (name, visible, mark) = (node.name.clone(), node.visible, node.export_body);
                        let splittable = self.scene.can_split_for_export(id);
                        ui.horizontal(|ui| {
                            ui.add_space(depth as f32 * 14.0);
                            let label = if visible {
                                theme::value(&name)
                            } else {
                                theme::hint(format!("{name} (hidden, not exported)"))
                            };
                            ui.add(egui::Label::new(label).selectable(false).truncate());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let mut chosen = mark;
                                egui::ComboBox::from_id_salt(("export-body", id))
                                    .width(150.0)
                                    .selected_text(body_label(mark))
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut chosen, None, body_label(None));
                                        for key in 1..=offered {
                                            let shared = Some(ExportBody::Shared(key));
                                            ui.selectable_value(&mut chosen, shared, body_label(shared));
                                        }
                                        if splittable {
                                            let split = Some(ExportBody::Split);
                                            ui.selectable_value(&mut chosen, split, body_label(split));
                                        }
                                    });
                                if chosen != mark {
                                    change = Some((id, chosen));
                                }
                            });
                        });
                    }
                });
            });
        ui.label(theme::hint(
            "A body of its own is one component. The same number on two shapes writes them as one solid. \
             Splitting a group offers what is inside it.",
        ));

        if let Some((id, body)) = change {
            self.edit("Group export bodies", None);
            self.scene.set_export_body(id, body);
        }
    }

    fn close_action(&mut self, ui: &mut egui::Ui) {
        if ui::dialog_button(ui, "Close", true).clicked() {
            self.modal = Modal::None;
        }
    }

    /// The keymap editor: every command grouped by area, with a search box, the
    /// current binding shown, and click-to-record (spec section 8.2).
    fn keymap_window(&mut self, ctx: &egui::Context) {
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

    fn keymap_body(&mut self, ui: &mut egui::Ui) {
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

    fn about_window(&mut self, ctx: &egui::Context) {
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

    fn about_body(&mut self, ui: &mut egui::Ui) {
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

    /// Failures are shown in a scrollable, copyable window with the specific
    /// reason, never a generic message (spec section 9).
    fn error_window(&mut self, ctx: &egui::Context) {
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

    fn error_body(&mut self, ui: &mut egui::Ui) {
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

    fn error_actions(&mut self, ui: &mut egui::Ui) {
        if ui::dialog_button(ui, "Close", true).clicked() {
            self.modal = Modal::None;
        }
        if ui::dialog_button(ui, "Copy", true).clicked() {
            ui.ctx().copy_text(self.error_detail.clone());
        }
    }

    /// Naming a group, or a whole project, before it goes on the palette.
    ///
    /// A window rather than an inline field because the name is going into the
    /// user's library, not into the document: it outlives this project, and it
    /// is the only thing the palette will show, so it is worth stopping to type.
    fn save_primitive_window(&mut self, ctx: &egui::Context) {
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

    fn save_primitive_body(&mut self, ui: &mut egui::Ui) {
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

    fn save_primitive_actions(&mut self, ui: &mut egui::Ui) {
        let named = !simple3d_core::library::sanitise(&self.primitive_name).is_empty();
        if ui::dialog_button(ui, "Save", named).clicked() {
            self.confirm_save_primitive();
        }
        if ui::dialog_button(ui, "Cancel", true).clicked() {
            self.cancel_save_primitive();
        }
    }

    /// The custom pattern kind creation tool (issue 67).
    fn pattern_kind_window(&mut self, ctx: &egui::Context) {
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

    /// The tool that cuts a shape into a pattern of pieces (issue 82).
    fn split_tool_window(&mut self, ctx: &egui::Context) {
        // Wide enough for the numbers and a picture of the cells beside them,
        // and no wider: unlike the pattern tool's, this preview is a flat plan
        // rather than a viewport, and it says what it has to say small.
        self.dialog(
            ctx,
            DialogSpec {
                key: "dialog-split-tool",
                title: "Split into smaller pieces",
                size: egui::vec2(680.0, 420.0),
                resizable: true,
                fit_height: false,
                min_size: Some(egui::vec2(320.0, 300.0)),
            },
            crate::split_tool::body,
            crate::split_tool::actions,
        );
    }

    fn confirm_close_tab_window(&mut self, ctx: &egui::Context) {
        self.dialog(
            ctx,
            DialogSpec {
                key: "dialog-confirm-close-tab",
                title: "Unsaved changes",
                size: egui::vec2(440.0, 150.0),
                resizable: false,
                fit_height: false,
                min_size: None,
            },
            Self::confirm_close_tab_body,
            Self::confirm_close_tab_actions,
        );
    }

    fn confirm_close_tab_body(&mut self, ui: &mut egui::Ui) {
        let name = self.pending_close.map(|index| self.tab_summary(index).0).unwrap_or_default();
        ui.label(format!("{name} has changes that have not been saved."));
        ui.add_space(4.0);
        ui.add(
            egui::Label::new(theme::hint("Saving writes it to its file; closing without saving discards it."))
                .selectable(false),
        );
    }

    fn confirm_close_tab_actions(&mut self, ui: &mut egui::Ui) {
        if ui::dialog_button(ui, "Save and close", true).clicked() {
            self.save_and_close_tab();
        }
        if ui::dialog_button(ui, "Close without saving", true).clicked() {
            self.confirm_close_tab();
        }
        cancel_at_left(ui, |ui| {
            if ui::dialog_button(ui, "Cancel", true).clicked() {
                self.cancel_close_tab();
            }
        });
    }

    fn confirm_quit_window(&mut self, ctx: &egui::Context) {
        self.dialog(
            ctx,
            DialogSpec {
                key: "dialog-confirm-quit",
                title: "Unsaved changes",
                size: egui::vec2(500.0, 150.0),
                resizable: false,
                fit_height: false,
                min_size: None,
            },
            Self::confirm_quit_body,
            Self::confirm_quit_actions,
        );
    }

    fn confirm_quit_body(&mut self, ui: &mut egui::Ui) {
        let others = (0..self.tab_count()).filter(|i| self.tab_summary(*i).1).count().saturating_sub(1);
        ui.label(match others {
            0 => "This project has changes that have not been saved.".to_string(),
            1 => "This project, and one other open document, have changes that have not been saved.".to_string(),
            n => format!("This project, and {n} other open documents, have changes that have not been saved."),
        });
        ui.add_space(4.0);
        ui.add(egui::Label::new(theme::hint("Only the document on screen can be saved from here.")).selectable(false));
    }

    fn confirm_quit_actions(&mut self, ui: &mut egui::Ui) {
        if ui::dialog_button(ui, "Save and quit", true).clicked() {
            self.save();
            if !self.unsaved() {
                self.confirm_quit();
            }
            self.modal = Modal::None;
        }
        if ui::dialog_button(ui, "Quit without saving", true).clicked() {
            self.confirm_quit();
            self.modal = Modal::None;
        }
        cancel_at_left(ui, |ui| {
            if ui::dialog_button(ui, "Cancel", true).clicked() {
                self.modal = Modal::None;
            }
        });
    }
}

/// A label that claims a whole column of a dialog's grid, so the rows below it
/// line up with the rows above and the fields beside them start in the same
/// place. A plain `ui.label` takes the width of its own text, which is what left
/// each grid measuring its own indent.
fn label_cell(ui: &mut egui::Ui, text: &str, width: f32) {
    ui.allocate_ui_with_layout(
        egui::vec2(width, theme::metric::INPUT_ROW),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            // The point of the cell is the width: without this the region
            // shrinks back to the text and the column is ragged again.
            ui.set_min_width(width);
            ui.add(egui::Label::new(text).truncate().selectable(false));
        },
    );
}

/// What a dialog's window is: what it is called, how big it opens and whether it
/// can be resized. One argument rather than four, so the call sites read as the
/// window they describe.
struct DialogSpec<'a> {
    key: &'a str,
    title: &'a str,
    /// The size the window opens at. With `fit_height` the height is only a
    /// starting point: the contents settle it on the first frame.
    size: egui::Vec2,
    resizable: bool,
    /// Take the height from the contents rather than from `size`.
    fit_height: bool,
    /// The smallest the window may be dragged to, for a resizable one whose
    /// contents stop making sense below a size. `None` leaves it to the window
    /// manager, which is right for a dialog that is a sentence and two buttons.
    min_size: Option<egui::Vec2>,
}

/// The buttons of a dialog, laid out the one way they are laid out everywhere:
/// along the foot of the window, right-aligned, the affirmative one last.
///
/// The contents are added *right to left*, so the closure names the rightmost
/// button first -- the one Enter would press if a dialog had a default -- and
/// the one that walks away from the dialog ends up furthest left, where the eye
/// arrives last.
fn action_row(ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui)) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = theme::metric::GAP * 2.0;
        contents(ui);
    });
}

/// Cancel, at the far end of a dialog's button row from the buttons that go
/// through with it. The row is laid out right to left, so what is left of it
/// after the other buttons is claimed here and filled left to right: the
/// button that abandons the dialog is not next to the one that commits it, and
/// cannot be hit by aiming for it.
fn cancel_at_left(ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui)) {
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), contents);
}

/// The status bar's separator: a dot, not a rule. A vertical line every few
/// words turns a single sentence of state into a row of boxes.
fn dot(ui: &mut egui::Ui) {
    ui.add(
        egui::Label::new(egui::RichText::new("\u{00B7}").size(theme::font::LABEL).color(theme::token::SURFACE_3))
            .selectable(false),
    );
}
