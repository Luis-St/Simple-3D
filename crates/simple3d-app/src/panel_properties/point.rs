//! A point in three fields, for the cursor and the view centre.

use super::*;
use crate::app::{App, Status};
use crate::theme::{self};
use simple3d_geom::Vec3;

/// A point's three components across the row, or one per line when too narrow (issue 51, as
/// `axis_row`). Clamping fields to a minimum width instead pushed Z past the panel edge.
pub(crate) fn point_fields(ui: &mut egui::Ui, name: &str, mut field: impl FnMut(&mut egui::Ui, usize)) {
    // The panel edge decides the share, and each field gets its colour-coded axis chip.
    let chip = theme::AXIS_CHIP_WIDTH + ui.spacing().item_spacing.x;
    let each = (room_left(ui) / 3.0 - chip - ui.spacing().item_spacing.x).min(POINT_FIELD_MAX);
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
        theme::axis_chip(ui, ui.id().with((name, axis)), axis);
        ui.scope(|ui| {
            ui.set_width(each);
            field(ui, axis);
        });
    }
}

/// A view centre rounded to 0.01 mm. The target is wherever a drag stopped, so four decimals
/// would not fit; a fixed two keep the readout stable. The camera keeps its exact value.
pub(crate) fn shown_view_centre(mm: f64) -> f64 {
    (mm * 100.0).round() / 100.0
}

/// The 3D cursor's position; `None` (never moved) means the origin, so the fields read zero.
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
                // Named like `axis_row`'s fields, so one cannot answer another's gesture.
                let grip = format!("3D cursor:{axis}");
                let field = Scalar { grip: &grip, id: field_id, kind: POINT, current: component(at, axis), step };
                // No undo step: the cursor is not part of the scene.
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

/// What the camera looks at, as three live fields, and a way back to the origin. It is also
/// where `Add at` places new shapes.
pub(crate) fn view_centre_rows(app: &mut App, ui: &mut egui::Ui) {
    let unit = app.unit();
    let at = app.scene.camera.target;
    let step = unit.from_mm(app.move_snap()).max(1e-6);
    // The hint names the zoom-hold key, since the wheel only moves this point while it is held (issue 97).
    let zoom_key = app.keymap.shortcut_text(simple3d_core::keymap::Command::ZoomToPointer);
    let towards = match zoom_key.is_empty() {
        true => String::new(),
        false => format!(" Hold {zoom_key} while zooming to walk it towards the pointer."),
    };
    field_row(
        ui,
        &named("View centre", unit.suffix()),
        &format!(
            "What the camera looks at: the point a pan carries about and an orbit turns around. \
             A new shape lands here when \u{201C}Add at\u{201D} is the view centre.{towards}"
        ),
        |ui| {
            // Laid out like the cursor's row. Locked, the fields are a greyed readout rather than inputs
            // that snap back.
            let locked = app.settings.lock_view_centre;
            point_fields(ui, "View centre", |ui, axis| {
                let field_id = ui.id().with(("view-centre", axis));
                let grip = format!("View centre:{axis}");
                // A stacked field's chip keeps its colour; only the number is greyed.
                if locked {
                    ui.disable();
                }
                let current = shown_view_centre(component(at, axis));
                let field = Scalar { grip: &grip, id: field_id, kind: POINT, current, step };
                // No undo step: no camera gesture records one, so typing a view centre does not either.
                scalar_field(app, ui, field, |app, mm, _| {
                    set_component(&mut app.scene.camera.target, axis, mm);
                });
            });
        },
    );
    field_row(ui, "", "", |ui| {
        // Separate buttons to lock and to move the point.
        let mut locked = app.settings.lock_view_centre;
        if theme::toggle(ui, &mut locked, "Lock")
            .on_hover_text(
                "Pin what the camera looks at. Orbit and zoom still work; a pan, a zoom held towards the \
                 pointer and these fields leave the view centre where it is.",
            )
            .changed()
        {
            app.settings.lock_view_centre = locked;
            app.status = Status::Info(if locked { "View centre locked" } else { "View centre unlocked" }.to_string());
        }
        // Deliberately not the cursor's "Back to the origin" wording, to avoid two identical labels.
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
