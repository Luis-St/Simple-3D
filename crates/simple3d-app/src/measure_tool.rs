//! The measure tool's window: the span it is holding, as numbers (issue 86).
//!
//! The tool itself is the pointer -- clicks in the viewport catch corners,
//! edges, face centres and axis crossings, and [`crate::app::Measure`] holds the
//! two ends they place. This is the other half of it: both ends as fields that
//! can be typed into, and what the span between them comes to.
//!
//! It lives in an [in-place popup](crate::popup) rather than in a section of the
//! properties panel, where it used to be. The panel describes *the selection*,
//! and a measurement has nothing to do with what happens to be selected: the
//! section appeared above the object being edited, pushed the rest of the panel
//! down, and went away again when the tool did. A tool with a state of its own
//! belongs in a window of its own, beside the split tool's -- floating over the
//! viewport, dragged where it is not in the way, rolled up when the model
//! underneath it matters more than the numbers, and closed by the cross that
//! closes every other one.
//!
//! Non-modal, like every popup here: the span is meant to be left on screen and
//! read while the model is orbited, which is the whole reason the numbers are
//! not in a dialog.

use crate::app::{App, Status};
use crate::panel_properties::{component, field_row, named, point_fields, scalar_field, set_component, Scalar, POINT};
use crate::popup::{self, PopupEvent, PopupSpec};
use crate::theme;
use crate::ui;
use simple3d_core::unit::{format_angle, format_length};
use simple3d_geom::Vec3;

/// Identifies the popup, and is what remembers where it was dragged to.
const KEY: &str = "measure-tool";

/// How wide the window is: three point fields across it and their axis chips,
/// and no wider. A popup lives over the model, so every pixel of it is a pixel
/// of the thing being measured that cannot be seen.
const WIDTH: f32 = 330.0;

/// The tool's window, drawn over the viewport once a frame while the tool is
/// out (issue 86).
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    if !app.measure.active {
        return;
    }
    let bounds = app.viewport_rect;
    // Taken out of the map for the duration, so the popup may hold it mutably
    // while its contents hold the application.
    let mut placement = app.popups.remove(KEY).unwrap_or_default();
    let event =
        popup::show(ctx, bounds, &mut placement, PopupSpec { key: KEY, title: "Measure", width: WIDTH }, |ui| {
            // Two point rows, three readings and a hint is a short window until the
            // rows stack on a narrow one, and a viewport can be short: the body
            // scrolls rather than pushing the buttons off the bottom of the screen.
            let (area, restore) = theme::list_scroll_area(ui);
            area.auto_shrink([false, true]).max_height(popup::body_room(bounds)).show(ui, |ui| {
                ui.set_style(restore);
                body(app, ui);
            });
            popup::action_row(ui, |ui| actions(app, ui));
        });
    app.popups.insert(KEY, placement);
    // The cross means the same thing the button in the row means: the tool is
    // put away, and the span with it.
    if event == PopupEvent::Closed && app.measure.active {
        app.toggle_measure();
    }
}

/// The span as numbers: both ends as editable fields, and the distance, per-axis
/// delta and angles between them (issues 69, 78).
///
/// The ends are editable because a measurement is often *between* named places
/// rather than between two things there is geometry to point at -- and because
/// having clicked one end approximately, correcting it by a tenth of a
/// millimetre should not mean clicking again and hoping.
pub(crate) fn body(app: &mut App, ui: &mut egui::Ui) {
    let unit = app.unit();
    let placed = app.measure.points.len();
    for (index, label) in [(0_usize, "Start"), (1, "End")] {
        let point = app.measure.points.get(index).copied();
        // An end can be typed only once the start is down; before that it would
        // be a point with nothing to measure to.
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
                // No undo step: the span belongs to the tool, not to the scene,
                // so there is no snapshot for one to restore.
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
            // A row each, where the panel put both on one. Two angles and their
            // names are wider than a popup, and a value that wraps under its own
            // label reads as a row that has gone wrong rather than as one number
            // followed by another.
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

/// The buttons along the foot: put the tool away, or keep it out and start the
/// span again.
pub(crate) fn actions(app: &mut App, ui: &mut egui::Ui) {
    if ui::dialog_button(ui, "Done", true).clicked() {
        app.toggle_measure();
    }
    // Clearing is at the other end of the row, the way a split's Cancel is: the
    // button that throws away what has been placed is as far as the window is
    // wide from the one that is pressed to finish.
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        // Named for what it clears: "Clear" beside a set of numbers is a
        // question about which of them.
        if ui.add_enabled(!app.measure.points.is_empty(), egui::Button::new("Clear the span")).clicked() {
            app.measure.clear();
            app.status = Status::Info("Measurement cleared".into());
        }
    });
}
