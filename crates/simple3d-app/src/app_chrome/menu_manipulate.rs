//! The Manipulate and Help menus.

use crate::app::{App, Modal, Status};
use crate::gizmo::Mode;
use crate::ui;
use simple3d_core::config::SnapMode;
use simple3d_core::keymap::Command;

impl App {
    pub(super) fn manipulate_menu(&mut self, ui: &mut egui::Ui) {
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

    pub(super) fn help_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Help", |ui| {
            if ui.button("About Simple 3D").clicked() {
                self.modal = Modal::About;
                ui.close();
            }
        });
    }
}
