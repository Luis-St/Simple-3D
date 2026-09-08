//! The Add and View menus.

use crate::app::App;
use crate::ui;
use simple3d_core::config::{DisplayMode, Panel, Side};
use simple3d_core::keymap::Command;
use simple3d_core::primitive;
use simple3d_core::scene::GroupOp;

impl App {
    /// Built entirely from the primitive registry: adding a primitive type puts
    /// it in this menu with no code here to change (spec section 3.2).
    pub(super) fn add_menu(&mut self, ui: &mut egui::Ui) {
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

    pub(super) fn view_menu(&mut self, ui: &mut egui::Ui) {
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
                (self.scene.settings.section.enabled, Command::ToggleSection, "Section view"),
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
}
