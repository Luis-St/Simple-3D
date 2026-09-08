//! What the measure tool has to say.

use super::*;
use crate::app::App;
use crate::theme::{self, token};
use crate::ui::{self};
use simple3d_core::scene::NodeId;
use simple3d_core::unit::format_length;

pub(crate) fn measurements(app: &mut App, ui: &mut egui::Ui, id: NodeId, selected: usize) {
    if selected > 1 {
        ui.add(
            egui::Label::new(theme::hint(format!("Measured from {}, the last one selected.", app.scene.node(id).name)))
                .selectable(false),
        );
    }
    let unit = app.unit();
    match app.evaluated.node_world_bounds.get(&id).copied() {
        Some((lo, hi)) => {
            field_row(ui, "Size", "", |ui| {
                ui.add(egui::Label::new(theme::numeric(ui::describe_size(hi - lo, unit))).selectable(false).wrap());
            });
            field_row(ui, "Centre", "", |ui| {
                ui.add(
                    egui::Label::new(theme::numeric(format!(
                        "{}, {}, {} {}",
                        format_length((lo.x + hi.x) / 2.0, unit),
                        format_length((lo.y + hi.y) / 2.0, unit),
                        format_length((lo.z + hi.z) / 2.0, unit),
                        unit.suffix()
                    )))
                    .selectable(false)
                    .wrap(),
                );
            });
        }
        None => {
            ui.add(egui::Label::new(theme::hint("No geometry yet.")).selectable(false));
        }
    }
    if let Some(error) = app.evaluated.error_for(id) {
        ui.colored_label(token::DANGER, &error.message);
    }
}
