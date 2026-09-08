//! The export dialog: format, scale and what is written.

use super::*;
use crate::app::{App, Modal};
use crate::ui;
use simple3d_core::unit::format_number;
use simple3d_export::{BodyMode, Format};

impl App {
    pub(super) fn export_window(&mut self, ctx: &egui::Context) {
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

    pub(super) fn export_body(&mut self, ui: &mut egui::Ui) {
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

    pub(super) fn export_actions(&mut self, ui: &mut egui::Ui) {
        if ui::dialog_button(ui, "Export...", true).clicked() {
            self.start_export();
        }
        if ui::dialog_button(ui, "Cancel", true).clicked() {
            self.modal = Modal::None;
        }
    }

    pub(super) fn export_controls(&mut self, ui: &mut egui::Ui) {
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
}
