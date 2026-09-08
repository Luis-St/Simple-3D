//! The rows placement is made of: the step it moves in, and one axis.

use super::*;
use crate::app::App;
use crate::theme::{self};
use crate::ui::{self};
use simple3d_core::primitive::ParamKind;

/// How far one step of a move or a resize goes, in the display unit.
///
/// It sits here, under the fields it governs, rather than only in the document
/// settings: the step is something you change *while* nudging something into
/// place, and going looking for it in another panel is the wrong five seconds.
/// The same value is on the Document panel, so it is also reachable with nothing
/// selected.
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

/// How far one step of a rotation turns, in degrees (issue 98).
///
/// Beside the move and resize step, and there for the same reason: the amount a
/// turn snaps to is something you change *while* bringing something round to
/// where it belongs. One number governs all of it -- the rotate handle's ring,
/// the nudge keys in rotate mode, the scrub on the rotation fields themselves,
/// and an angle a pattern drives -- and until now it could only be reached by
/// editing the settings file, which is why 15 degrees read as fixed.
///
/// Unlike the step above it this is a user setting, not a document one: a
/// distance only means something against the size of what is being built, but
/// fifteen degrees is fifteen degrees in every project.
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
            // No undo step: it is a setting of the application, not of the
            // document, so the history has nothing to say about it. The frame
            // loop writes it to the settings file as it does every other one.
            scalar_field(app, ui, field, |app, degrees, _started| {
                app.settings.rotate_snap_deg = degrees;
            });
        });
    });
}

/// One labelled row of three axis fields, each preceded by its colour chip. The
/// chip is handed to the caller as that field's scrub grip.
pub(crate) fn axis_row(
    app: &mut App,
    ui: &mut egui::Ui,
    label: &str,
    mut field: impl FnMut(&mut App, &mut egui::Ui, usize, &str),
) {
    field_row(ui, label, "", |ui| {
        // Three fields, three chips, and the gaps between them all have to come
        // out of the row: getting this wrong pushes the Z field off the panel.
        // The panel's own edge decides how much there is to share out, never the
        // widest row above -- one row wide enough to overflow would otherwise
        // take Z with it.
        let available = room_left(ui);
        let chips = 3.0 * (theme::AXIS_CHIP_WIDTH + ui.spacing().item_spacing.x);
        let gaps = 2.0 * ui.spacing().item_spacing.x;
        let each = (available - chips - gaps) / 3.0;
        // Below a width where a number is still readable, the three axes go one
        // to a line at full width rather than three unusable slivers (issue 51).
        // Each keeps its colour chip, which is what says which axis it is.
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
            // The chip is a label, not a handle: it says which axis this column
            // is, and the field beside it carries the drag.
            theme::axis_chip(ui, ui.id().with((label, axis)), axis);
            let name = format!("{label}:{axis}");
            ui.scope(|ui| {
                ui.set_width(each);
                field(app, ui, axis, &name);
            });
        }
    });
}
