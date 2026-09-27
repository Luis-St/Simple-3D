//! The measure tool's window: its span as numbers (issue 86).
//!
//! The pointer places the two ends in the viewport ([`crate::app::Measure`]); this window shows
//! them as editable fields with the span's measurements. A non-modal
//! [in-place popup](crate::popup) rather than a properties section, since a measurement is
//! unrelated to the selection and is meant to stay readable while orbiting.

use crate::app::{App, Status};
use crate::panel_properties::{component, field_row, named, point_fields, scalar_field, set_component, Scalar, POINT};
use crate::popup::{self, PopupEvent, PopupSpec};
use crate::theme;
use crate::ui;
use simple3d_core::unit::{format_angle, format_length};
use simple3d_geom::Vec3;

/// The popup's key, which also remembers where it was dragged.
const KEY: &str = "measure-tool";

/// The window width: three point fields and their chips, no wider, since it covers the model.
const WIDTH: f32 = 330.0;

/// The tool's window over the viewport, while the tool is out (issue 86).
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    if !app.measure.active {
        return;
    }
    let bounds = app.viewport_rect;
    // Taken out of the map so the popup can hold it mutably while the contents hold the app.
    let mut placement = app.popups.remove(KEY).unwrap_or_default();
    let event =
        popup::show(ctx, bounds, &mut placement, PopupSpec { key: KEY, title: "Measure", width: WIDTH }, |ui| {
            // Scrolls on short viewports so the buttons stay reachable.
            popup::scrolling_body(ui, bounds, |ui| body(app, ui));
            popup::action_row(ui, |ui| actions(app, ui));
        });
    app.popups.insert(KEY, placement);
    // The cross puts the tool and its span away, like the row's button.
    if event == PopupEvent::Closed && app.measure.active {
        app.toggle_measure();
    }
}

/// The span as numbers: both ends as editable fields, plus distance, per-axis delta and angles
/// (issues 69, 78). Editable so an end can be typed or corrected precisely.
pub(crate) fn body(app: &mut App, ui: &mut egui::Ui) {
    let unit = app.unit();
    let placed = app.measure.points.len();
    for (index, label) in [(0_usize, "Start"), (1, "End")] {
        let point = app.measure.points.get(index).copied();
        // The end is editable only once the start is placed.
        let enabled = index <= placed;
        let at = point.map_or(Vec3::ZERO, |p| p.at);
        let step = unit.from_mm(app.move_snap()).max(1e-6);
        let hover = match point.and_then(|p| p.kind) {
            Some(kind) => format!("Caught the {} of a body. Type here to place it exactly.", kind.label()),
            None if point.is_some() => "Click in the viewport to move it, or type it exactly.".to_string(),
            None => "Click in the viewport to place it, or type it here.".to_string(),
        };
        field_row(ui, &named(label, unit.suffix()), &hover, |ui| {
            point_fields(ui, label, |ui, axis| {
                let field_id = ui.id().with(("measure", index, axis));
                let grip = format!("{label}:{axis}");
                if !enabled {
                    ui.disable();
                }
                let field = Scalar { grip: &grip, id: field_id, kind: POINT, current: component(at, axis), step };
                // No undo step: the span belongs to the tool, not the scene.
                scalar_field(app, ui, field, |app, mm, _| {
                    let mut p = app.measure.points.get(index).map_or(Vec3::ZERO, |p| p.at);
                    set_component(&mut p, axis, mm);
                    app.measure.set_point(index, p);
                });
            });
        });
    }

    match app.measure.span() {
        Some((a, b)) => {
            let m = crate::app::Measurement::between(a.at, b.at);
            let suffix = unit.suffix();
            field_row(ui, "Distance", "", |ui| {
                ui.add(
                    egui::Label::new(theme::numeric(format!("{} {suffix}", format_length(m.distance, unit))))
                        .selectable(false)
                        .wrap(),
                );
            });
            field_row(ui, "\u{0394}", "The span, axis by axis", |ui| {
                ui.add(
                    egui::Label::new(theme::numeric(format!(
                        "{}, {}, {} {suffix}",
                        format_length(m.delta.x, unit),
                        format_length(m.delta.y, unit),
                        format_length(m.delta.z, unit)
                    )))
                    .selectable(false)
                    .wrap(),
                );
            });
            // One row per angle, since two on one line are wider than the popup.
            for (label, hover, angle) in [
                ("Incline", "Above the ground plane", m.inclination_deg),
                ("Bearing", "Around the ground plane, from +X towards +Y", m.bearing_deg),
            ] {
                field_row(ui, label, hover, |ui| {
                    ui.add(
                        egui::Label::new(theme::numeric(format!("{}\u{00B0}", format_angle(angle))))
                            .selectable(false)
                            .wrap(),
                    );
                });
            }
        }
        None => {
            ui.add(
                egui::Label::new(theme::hint(
                    "Click two features in the viewport. The pointer catches corners, edges, face centres, the \
                     marks the planes through zero leave on a body, and the axes themselves -- whatever the frame \
                     shows; right-click takes the last one back.",
                ))
                .selectable(false),
            );
        }
    }
}

/// Footer buttons: put the tool away, or clear the span.
pub(crate) fn actions(app: &mut App, ui: &mut egui::Ui) {
    if ui::dialog_button(ui, "Done", true).clicked() {
        app.toggle_measure();
    }
    // Clearing sits at the other end of the row from finishing, like a split's Cancel.
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        // Named for what it clears.
        if ui.add_enabled(!app.measure.points.is_empty(), egui::Button::new("Clear the span")).clicked() {
            app.measure.clear();
            app.status = Status::Info("Measurement cleared".into());
        }
    });
}
