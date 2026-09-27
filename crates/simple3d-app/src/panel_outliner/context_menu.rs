//! The menu a right click opens on a row.

use crate::app::App;
use simple3d_core::keymap::Keymap;
use simple3d_core::scene::{Colour, ComponentId, GroupOp, NodeId, ROOT_COMPONENT};

/// The right-click menu on an outliner row.
///
/// Deliberately the same commands as the menu bar, with their shortcuts, so they can be learned here.
pub(crate) fn context_menu(app: &mut App, response: &egui::Response, id: NodeId, is_root: bool) {
    let is_group = app.scene.node(id).is_group();
    use simple3d_core::keymap::Command;

    /// Dividers between the menu's blocks, drawn only once something follows, so there are never
    /// two in a row or one at the bottom.
    struct Blocks {
        started: bool,
        ruled: bool,
    }

    impl Blocks {
        /// A block ends here; the next entry starts a new one.
        fn rule(&mut self) {
            self.ruled = true;
        }

        fn entry(&mut self, ui: &mut egui::Ui) {
            if self.ruled && self.started {
                ui.separator();
            }
            self.ruled = false;
            self.started = true;
        }
    }

    /// One command in the menu, omitted where it would do nothing. The choice is collected rather
    /// than run: labelling borrows the keymap, running needs the whole application.
    fn item(
        ui: &mut egui::Ui,
        blocks: &mut Blocks,
        keymap: &Keymap,
        chosen: &mut Option<Command>,
        command: Command,
        available: bool,
    ) {
        if !available {
            return;
        }
        blocks.entry(ui);
        if crate::ui::menu_entry(ui, &crate::ui::menu_label(keymap, command), true).clicked() {
            *chosen = Some(command);
            ui.close();
        }
    }

    response.context_menu(|ui| {
        // Right-clicking an unselected row acts on that row, so a delete never hits the wrong node.
        if !app.is_selected(id) {
            app.select_only(id);
        }
        let hidden = !app.scene.node(id).visible;
        let multiple = app.selection.len() > 1;
        let have_clipboard = app.clipboard.is_some();

        // Named by effect, since "Toggle visibility" would be ambiguous.
        let show_hide = format!(
            "{}\t{}",
            if hidden { "Show" } else { "Hide" },
            app.keymap.shortcut_text(Command::ToggleVisibility)
        );

        let mut chosen: Option<Command> = None;
        let mut operation: Option<GroupOp> = None;
        let mut add: Option<(Option<&'static str>, GroupOp)> = None;
        let mut add_pattern = false;
        let mut add_custom_pattern = false;
        let mut save_as_primitive = false;
        let mut open_component = false;
        let mut place_component: Option<ComponentId> = None;
        // Every component placeable here, with the reason where one cannot be.
        let placeable: Vec<(ComponentId, String, Option<String>)> = app
            .project
            .components
            .iter()
            .filter(|c| c.id != ROOT_COMPONENT)
            .map(|c| (c.id, app.component_label(c.id).unwrap_or_default(), app.can_integrate(c.id).err()))
            .collect();
        let is_component = app.scene.node(id).is_component();
        let uses_components = app.project.uses_components();
        let mut paint: Option<Option<Colour>> = None;
        let keymap = &app.keymap;
        let mut blocks = Blocks { started: false, ruled: false };

        // First, as adding is the most common use; a shape goes inside the clicked group or beside
        // anything else (issue 44).
        blocks.entry(ui);
        ui.menu_button("Add", |ui| {
            for op in GroupOp::ALL {
                if ui.button(format!("{} group", op.label())).clicked() {
                    add = Some((None, op));
                    ui.close();
                }
            }
            // Separates combining containers (groups) from repeating ones (patterns).
            ui.separator();
            // An empty pattern (issue 67); wrapping the selection is further down.
            if ui
                .button("Pattern")
                .on_hover_text("An empty pattern, for shapes to be put into it and repeated")
                .clicked()
            {
                add_pattern = true;
                ui.close();
            }
            // The custom-pattern tool (issue 67), next to "Pattern" so it can be found.
            if ui
                .button("Custom pattern")
                .on_hover_text("Build a repetition rule out of stages, and keep it for other projects")
                .clicked()
            {
                add_custom_pattern = true;
                ui.close();
            }
            ui.separator();
            // The project's components as their own category (issue 113), once there is one to place.
            if !placeable.is_empty() {
                ui.menu_button("Components", |ui| {
                    for (component, name, refusal) in &placeable {
                        let button = ui.add_enabled(refusal.is_none(), egui::Button::new(name));
                        if let Some(why) = refusal {
                            button.on_disabled_hover_text(why);
                        } else if button.clicked() {
                            place_component = Some(*component);
                            ui.close();
                        }
                    }
                });
                ui.separator();
            }
            for category in simple3d_core::primitive::categories() {
                ui.menu_button(category, |ui| {
                    for spec in simple3d_core::primitive::REGISTRY.iter().filter(|s| s.category == category) {
                        if ui.button(spec.label).clicked() {
                            add = Some((Some(spec.type_id), GroupOp::Union));
                            ui.close();
                        }
                    }
                });
            }
        })
        .response
        .on_hover_text(if app.scene.node(id).is_pattern() {
            "Into this pattern, to be repeated"
        } else if is_group {
            "Into this group"
        } else {
            "Beside this node"
        });
        blocks.rule();
        item(ui, &mut blocks, keymap, &mut chosen, Command::Rename, (!is_root || uses_components) && !multiple);
        item(ui, &mut blocks, keymap, &mut chosen, Command::Duplicate, !is_root);
        blocks.rule();
        item(ui, &mut blocks, keymap, &mut chosen, Command::Copy, !is_root);
        item(ui, &mut blocks, keymap, &mut chosen, Command::Cut, !is_root);
        item(ui, &mut blocks, keymap, &mut chosen, Command::Paste, have_clipboard);
        blocks.rule();
        // What a node can be put into (issue 94: each block is one kind of action).
        item(ui, &mut blocks, keymap, &mut chosen, Command::Group, !is_root);
        item(ui, &mut blocks, keymap, &mut chosen, Command::Pattern, !is_root);
        // Make a group into a component, or open an existing one (issue 113).
        item(ui, &mut blocks, keymap, &mut chosen, Command::MakeComponent, is_group && !is_root && !multiple);
        // On the root an empty component, on other groups one made from the group.
        item(ui, &mut blocks, keymap, &mut chosen, Command::NewComponent, is_group && !multiple);
        if is_component && !multiple {
            blocks.entry(ui);
            if crate::ui::menu_entry(ui, "Open the component", true).clicked() {
                open_component = true;
                ui.close();
            }
        }
        blocks.rule();
        // What a node can be turned into: mesh (issue 80), simplify (issue 106), split, also re-cutting
        // a split (issue 82), and unsplit.
        item(ui, &mut blocks, keymap, &mut chosen, Command::ConvertToMesh, !is_root && !app.scene.node(id).is_mesh());
        item(ui, &mut blocks, keymap, &mut chosen, Command::SimplifyMesh, !multiple && app.scene.node(id).is_mesh());
        item(ui, &mut blocks, keymap, &mut chosen, Command::Reassemble, !multiple && app.scene.node(id).is_mesh());
        item(ui, &mut blocks, keymap, &mut chosen, Command::SplitIntoPieces, !is_root && !multiple);
        item(ui, &mut blocks, keymap, &mut chosen, Command::Rejoin, !multiple && app.scene.node(id).is_split());
        blocks.rule();
        // Omitted rather than silent where the move has nowhere to go (issue 41).
        item(ui, &mut blocks, keymap, &mut chosen, Command::MoveUp, app.can_reorder(-1));
        item(ui, &mut blocks, keymap, &mut chosen, Command::MoveDown, app.can_reorder(1));
        // A group's operator (issue 37), omitted rather than greyed out for non-groups.
        blocks.rule();
        if is_group {
            blocks.entry(ui);
            let current = app.scene.node(id).group_op();
            ui.menu_button("Operation", |ui| {
                for option in GroupOp::ALL {
                    if crate::ui::menu_entry(ui, option.label(), current != Some(option)).clicked() {
                        operation = Some(option);
                        ui.close();
                    }
                }
            });
        }
        blocks.rule();
        if !is_root {
            blocks.entry(ui);
            if crate::ui::menu_entry(ui, &show_hide, true).clicked() {
                chosen = Some(Command::ToggleVisibility);
                ui.close();
            }
        }
        blocks.rule();
        // Painting here too, since groups are easiest to point at in the outliner. Plain buttons, not
        // a picker: a picker popup would close the menu before a colour is chosen.
        blocks.entry(ui);
        ui.label(crate::theme::hint("Colour"));
        ui.horizontal(|ui| {
            for (name, preset) in crate::theme::PAINT_PRESETS {
                let swatch = egui::Button::new("")
                    .fill(preset)
                    .stroke(egui::Stroke::new(1.0_f32, crate::theme::token::SURFACE_3))
                    .min_size(egui::vec2(16.0, 16.0));
                if ui.add(swatch).on_hover_text(name).clicked() {
                    paint = Some(Some(Colour([preset.r(), preset.g(), preset.b()])));
                    ui.close();
                }
            }
        });
        // The user's custom recent colours; presets are already in the row above.
        let recent: Vec<[u8; 3]> = app.custom_recent_colours();
        if !recent.is_empty() {
            ui.horizontal(|ui| {
                for colour in recent {
                    let swatch = egui::Button::new("")
                        .fill(egui::Color32::from_rgb(colour[0], colour[1], colour[2]))
                        .stroke(egui::Stroke::new(1.0_f32, crate::theme::token::SURFACE_3))
                        .min_size(egui::vec2(16.0, 16.0));
                    let name = format!("#{:02x}{:02x}{:02x}", colour[0], colour[1], colour[2]);
                    if ui.add(swatch).on_hover_text(name).clicked() {
                        paint = Some(Some(Colour(colour)));
                        ui.close();
                    }
                }
            });
        }
        if app.scene.subtree_is_painted(id)
            && ui
                .button("Clear the colour")
                .on_hover_text("Back to the theme's colour for an unpainted solid")
                .clicked()
        {
            paint = Some(None);
            ui.close();
        }
        blocks.rule();
        blocks.entry(ui);
        if ui
            .button("Save as primitive\u{2026}")
            .on_hover_text("Keep this, and everything under it, on the palette to use in any project")
            .clicked()
        {
            save_as_primitive = true;
            ui.close();
        }
        blocks.rule();
        item(ui, &mut blocks, &app.keymap, &mut chosen, Command::Delete, !is_root);

        if let Some(op) = operation {
            app.set_group_op(id, op);
        }
        if let Some((type_id, op)) = add {
            app.add_node_at(id, type_id, op);
        }
        if add_pattern {
            app.add_pattern_at(id);
        }
        if let Some(component) = place_component {
            app.integrate_component_at(id, component);
        }
        // Add an empty pattern and open the tool on it, so it is custom from the start.
        if add_custom_pattern {
            app.add_pattern_at(id);
            if let Some(made) = app.primary() {
                app.open_pattern_tool_on_new(made);
            }
        }
        if let Some(colour) = paint {
            let targets: Vec<NodeId> = app.selection.to_vec();
            app.paint(&targets, colour, None);
        }
        if save_as_primitive {
            app.save_selection_as_primitive();
        }
        if open_component {
            app.open_component_of(id);
        }
        if let Some(command) = chosen {
            app.run(command);
        }
    });
}
