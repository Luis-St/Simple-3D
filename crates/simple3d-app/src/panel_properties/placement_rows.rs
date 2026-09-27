//! The placement rows: the step it moves in, and one axis.

use super::*;
use crate::app::App;
use crate::theme::{self};
use crate::ui::{self};
use simple3d_core::primitive::ParamKind;

/// The move and resize step in the display unit, placed under the fields it governs since it is
/// changed mid-nudge; also on the Document panel.
pub fn step_row(app: &mut App, ui: &mut egui::Ui) {
    let unit = app.unit();
    let hover = "How far one nudge, and one snapped step of a move or resize drag, goes.";
    field_row(ui, &named("Step", unit.suffix()), hover, |ui| {
        let kind = ParamKind::Length { min: 1e-6 };
        let snap = app.scene.settings.snap_step;
        let width = room_left(ui).max(40.0);
        let field_id = ui.id().with("doc-step");
        ui.scope(|ui| {
            ui.set_width(width);
            let field =
                Scalar { grip: "Step", id: field_id, kind, current: snap, step: ui::scrub_increment(kind, unit) };
            scalar_field(app, ui, field, |app, mm, started| {
                edit_or_touch(app, started, "Step", "scene:step");
                app.scene.settings.snap_step = mm.min(MAX_LENGTH);
            });
        });
    });
}

/// The rotation step in degrees (issue 98), governing the ring, rotate-mode nudges, rotation
/// scrubs and pattern angles; previously only in the settings file. A user setting, since degrees
/// mean the same in every project.
pub fn rotate_step_row(app: &mut App, ui: &mut egui::Ui) {
    let hover = "How far one nudge, and one snapped step of a rotate drag, turns.";
    field_row(ui, "Turn (deg)", hover, |ui| {
        let width = room_left(ui).max(40.0);
        let field_id = ui.id().with("doc-rotate-step");
        ui.scope(|ui| {
            ui.set_width(width);
            let field = Scalar {
                grip: "Turn",
                id: field_id,
                kind: ROTATE_STEP,
                current: app.settings.rotate_snap_deg,
                step: 1.0,
            };
            // No undo step: an application setting, written by the frame loop like the others.
            scalar_field(app, ui, field, |app, degrees, _started| {
                app.settings.rotate_snap_deg = degrees;
            });
        });
    });
}

/// One labelled row of three axis fields, each with its colour chip.
pub(crate) fn axis_row(
    app: &mut App,
    ui: &mut egui::Ui,
    label: &str,
    mut field: impl FnMut(&mut App, &mut egui::Ui, usize, &str),
) {
    field_row(ui, label, "", |ui| {
        // The fields, chips and gaps share the panel's width, not the widest row's, or Z is pushed off.
        let available = room_left(ui);
        let chips = 3.0 * (theme::AXIS_CHIP_WIDTH + ui.spacing().item_spacing.x);
        let gaps = 2.0 * ui.spacing().item_spacing.x;
        let each = (available - chips - gaps) / 3.0;
        // Below readable width the axes stack one per line, each keeping its chip (issue 51).
        if each < MIN_AXIS_FIELD {
            ui.vertical(|ui| {
                for axis in 0..3 {
                    ui.horizontal(|ui| {
                        theme::axis_chip(ui, ui.id().with((label, axis)), axis);
                        let name = format!("{label}:{axis}");
                        field(app, ui, axis, &name);
                    });
                }
            });
            return;
        }
        for axis in 0..3 {
            // The chip labels the axis; the field carries the drag.
            theme::axis_chip(ui, ui.id().with((label, axis)), axis);
            let name = format!("{label}:{axis}");
            ui.scope(|ui| {
                ui.set_width(each);
                field(app, ui, axis, &name);
            });
        }
    });
}
