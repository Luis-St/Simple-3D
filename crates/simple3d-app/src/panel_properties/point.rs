//! A point in three fields, for the cursor and the view centre.

use super::*;
use crate::app::{App, Status};
use crate::theme::{self};
use simple3d_geom::Vec3;

/// The three components of a point -- the 3D cursor, the view centre, an end of
/// the measure span -- laid out across the row, and one to a line when the row
/// is too narrow to hold three fields across it.
///
/// The narrow layout is `axis_row`'s, and for the same reason (issue 51): three
/// fields plus their gaps need more than a third of the row each, so below the
/// width where a number is still readable the only way to keep all three on the
/// panel is to give each its own line. Each then carries the axis chip that says
/// which one it is -- across the row their order says it, stacked it does not.
///
/// They were clamped to a floor of 44 points instead, which is not a layout:
/// three fields of it and their gaps are wider than the panel that forced them
/// there, so the third was drawn past the panel's edge and clipped. That is what
/// was reported -- the 3D cursor and the view centre losing their Z field as the
/// dock was dragged in, while the position and rotation rows above them stacked.
pub(crate) fn point_fields(ui: &mut egui::Ui, name: &str, mut field: impl FnMut(&mut egui::Ui, usize)) {
    // The panel's own edge decides how much there is to share out, the way it
    // does on an axis row: a row wide enough to overflow must not take the
    // others with it.
    let each = (room_left(ui) / 3.0 - ui.spacing().item_spacing.x).min(POINT_FIELD_MAX);
    if each < MIN_AXIS_FIELD {
        ui.vertical(|ui| {
            for axis in 0..3 {
                ui.horizontal(|ui| {
                    theme::axis_chip(ui, ui.id().with((name, axis)), axis);
                    field(ui, axis);
                });
            }
        });
        return;
    }
    for axis in 0..3 {
        ui.scope(|ui| {
            ui.set_width(each);
            field(ui, axis);
        });
    }
}

/// A view centre to a hundredth of a millimetre, at whatever magnitude.
///
/// Every other number in the panel is a measurement, and a measurement is shown
/// to the last place that round-trips -- four decimals in millimetres. The
/// camera's target is not a measurement: it is wherever a drag happened to stop,
/// so it carries all four of those places nearly all of the time, and
/// `-8.2888` in a field sized for `40` is the number that would not fit.
///
/// Two decimals everywhere rather than fewer as the number grows: a readout the
/// eye can compare from one frame to the next is one whose shape does not change
/// under it, and the far end of the camera's range is somewhere a view visits,
/// not somewhere it works. This is what the field displays and what a scrub
/// counts from; the camera itself keeps whatever a drag left it at.
pub(crate) fn shown_view_centre(mm: f64) -> f64 {
    (mm * 100.0).round() / 100.0
}

/// Where the 3D cursor is, to the millimetre.
///
/// The cursor is `None` when it has never been moved, which means the origin,
/// so the fields read zero and typing into one places it: the same two states
/// the viewport draws, without a third way of saying "nowhere".
pub(crate) fn cursor_rows(app: &mut App, ui: &mut egui::Ui) {
    let unit = app.unit();
    let at = app.cursor.unwrap_or(Vec3::ZERO);
    let step = unit.from_mm(app.move_snap()).max(1e-6);
    field_row(
        ui,
        &named("3D cursor", unit.suffix()),
        "Where a new shape lands when \u{201C}Add at\u{201D} is the cursor. \
             Shift+right-click in the viewport puts it under the pointer.",
        |ui| {
            point_fields(ui, "3D cursor", |ui, axis| {
                let field_id = ui.id().with(("cursor", axis));
                // Named the way `axis_row` names its three, so one field cannot
                // answer to another's gesture.
                let grip = format!("3D cursor:{axis}");
                let field = Scalar { grip: &grip, id: field_id, kind: POINT, current: component(at, axis), step };
                // No undo step: the cursor is not part of the scene, so there is
                // no snapshot for one to restore.
                scalar_field(app, ui, field, |app, mm, _| {
                    let mut p = app.cursor.unwrap_or(Vec3::ZERO);
                    set_component(&mut p, axis, mm);
                    app.cursor = Some(p);
                });
            });
        },
    );
    field_row(ui, "", "", |ui| {
        if ui
            .add_enabled(app.cursor.is_some(), egui::Button::new("Back to the origin"))
            .on_hover_text("The cursor goes back to 0, 0, 0")
            .clicked()
        {
            app.cursor = None;
            app.status = Status::Info("3D cursor back at the origin".into());
        }
    });
}

/// What the camera is looking at, as three numbers, and a way back to the
/// origin.
///
/// The viewport is the usual way to move it -- a middle drag carries it across
/// the ground and the wheel walks it towards the pointer -- but no gesture says
/// *exactly* here, and once the view has wandered off the model none of them
/// says "back to the middle of everything" either. It is a number the document
/// already works from: `Add at` places a new shape at the view centre, and this
/// is the row that says where that is.
///
/// Read as well as written: it follows a pan or a zoom live, so it is also the
/// answer to "where am I looking?".
pub(crate) fn view_centre_rows(app: &mut App, ui: &mut egui::Ui) {
    let unit = app.unit();
    let at = app.scene.camera.target;
    let step = unit.from_mm(app.move_snap()).max(1e-6);
    field_row(
        ui,
        &named("View centre", unit.suffix()),
        "What the camera looks at: the point a pan carries about and an orbit turns around. \
             A new shape lands here when \u{201C}Add at\u{201D} is the view centre.",
        |ui| {
            // Laid out like the 3D cursor's row above, narrow row included: they
            // are the same kind of thing and read as a pair.
            //
            // Locked, they are a readout: still shown, still following the
            // camera, but greyed and inert. A field that takes a number and then
            // puts it back is worse than one that says it will not.
            let locked = app.settings.lock_view_centre;
            point_fields(ui, "View centre", |ui, axis| {
                let field_id = ui.id().with(("view-centre", axis));
                let grip = format!("View centre:{axis}");
                // The chip beside a stacked field keeps its colour: what is
                // greyed is the number that cannot be typed into.
                if locked {
                    ui.disable();
                }
                let current = shown_view_centre(component(at, axis));
                let field = Scalar { grip: &grip, id: field_id, kind: POINT, current, step };
                // No undo step: where the camera looks is not part of the scene
                // the history holds, and neither pan nor orbit nor the wheel
                // records one either. Typing a view centre is the same gesture by
                // another route, so it cannot be the one thing about the camera
                // that Ctrl+Z takes back.
                scalar_field(app, ui, field, |app, mm, _| {
                    set_component(&mut app.scene.camera.target, axis, mm);
                });
            });
        },
    );
    field_row(ui, "", "", |ui| {
        // Two things, so two buttons: one pins the point the camera turns about,
        // the other moves it. They were one button, and a button that both
        // locked and moved a value would be neither.
        let mut locked = app.settings.lock_view_centre;
        if theme::toggle(ui, &mut locked, "Lock")
            .on_hover_text(
                "Pin what the camera looks at. Orbit and zoom still work; a pan, a zoom about the pointer \
                 and these fields leave the view centre where it is.",
            )
            .changed()
        {
            app.settings.lock_view_centre = locked;
            app.status = Status::Info(if locked { "View centre locked" } else { "View centre unlocked" }.to_string());
        }
        // Deliberately not the cursor's "Back to the origin" wording, three rows
        // above: two buttons with one label in the same section, each belonging
        // to a different row, is a coin toss rather than a choice.
        let away = app.scene.camera.target != Vec3::ZERO;
        if ui
            .add_enabled(away && !locked, egui::Button::new("Reset to origin"))
            .on_hover_text("The camera looks at 0, 0, 0 again, from the angle and distance it is at now")
            .clicked()
        {
            app.scene.camera.target = Vec3::ZERO;
            app.status = Status::Info("View centre back at the origin".into());
        }
    });
}
