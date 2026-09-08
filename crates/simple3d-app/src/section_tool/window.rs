//! The tool's own window.

use super::*;
use crate::app::App;
use crate::panel_properties::{component, field_row, named, room_left, scalar_field, Scalar, POINT};
use crate::popup::{self, PopupEvent, PopupSpec};
use crate::theme;
use crate::ui;
use simple3d_core::keymap::Command;
use simple3d_core::unit::format_length;
use simple3d_geom::Vec3;

/// The section's window, drawn over the viewport once a frame while the plane
/// is out (issue 72).
///
/// It stands open for exactly as long as the section is on -- there is no
/// separate switch for the window, because a section with its settings put away
/// and a section that is off are the same picture. The chevron is what puts the
/// window out of the way while the cut stays.
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    if !app.scene.settings.section.enabled {
        return;
    }
    let bounds = app.viewport_rect;
    // Taken out of the map for the duration, so the popup may hold it mutably
    // while its contents hold the application.
    let mut placement = app.popups.remove(KEY).unwrap_or_default();
    let event =
        popup::show(ctx, bounds, &mut placement, PopupSpec { key: KEY, title: "Section", width: WIDTH }, |ui| {
            // Four rows and a hint is a short window until the rows stack on a
            // narrow one, and a viewport can be short: the body scrolls rather
            // than pushing the button off the bottom of the screen.
            let (area, restore) = theme::list_scroll_area(ui);
            area.auto_shrink([false, true]).max_height(popup::body_room(bounds)).show(ui, |ui| {
                ui.set_style(restore);
                body(app, ui);
            });
            popup::action_row(ui, |ui| actions(app, ui));
        });
    app.popups.insert(KEY, placement);
    // The cross means the same thing the button in the row means: the plane is
    // put away and the model is whole again.
    if event == PopupEvent::Closed && app.scene.settings.section.enabled {
        app.run(Command::ToggleSection);
    }
}

/// The plane's settings: the axis it stands perpendicular to, where along that
/// axis it sits, and which side of it is cut away (issues 71, 72).
///
/// The offset is a scalar field like any other, so it can be typed exactly and
/// scrubbed with the pointer -- and the grips in the viewport slide the same
/// number. A plane that can only be dragged cannot be put at 12.5 mm, and one
/// that can only be typed cannot be swept through a part to find where the wall
/// gets thin.
pub(crate) fn body(app: &mut App, ui: &mut egui::Ui) {
    let unit = app.unit();
    field_row(ui, "Plane", "The axis the section plane stands perpendicular to", |ui| {
        for (axis, name) in ["X", "Y", "Z"].into_iter().enumerate() {
            let showing = app.scene.settings.section.axis() == axis;
            if theme::choice(ui, showing, name).clicked() && !showing {
                app.scene.settings.section.axis = axis;
                // The old offset is a place on a different axis, so the plane
                // goes back to the middle of the model rather than to wherever
                // that number happens to land on this one.
                let middle = middle_of(app.evaluated.mesh.bounds(), axis);
                app.set_section_offset(middle);
            }
        }
    });
    field_row(ui, &named("At", unit.suffix()), "Where the plane sits along its axis", |ui| {
        let step = unit.from_mm(app.move_snap()).max(1e-6);
        let width = room_left(ui).max(40.0);
        let id = ui.id().with("section-offset");
        ui.scope(|ui| {
            ui.set_width(width);
            let field = Scalar { grip: "Section", id, kind: POINT, current: app.scene.settings.section.offset, step };
            // No undo step: moving the plane is not an edit -- see the module
            // header.
            scalar_field(app, ui, field, |app, mm, _| app.set_section_offset(mm));
        });
    });
    field_row(ui, "Keeps", "Which side of the plane stays in the picture", |ui| {
        // Named by the coordinate, not by the camera: which side is nearer
        // depends on where the model has been orbited to.
        for (flipped, label) in [(false, "Below"), (true, "Above")] {
            let showing = app.scene.settings.section.flipped == flipped;
            if theme::choice(ui, showing, label).clicked() && !showing {
                app.scene.settings.section.flipped = flipped;
                app.status = crate::app::Status::Info(readout(app));
            }
        }
    });
    ui.add(
        egui::Label::new(theme::hint(
            "The cut is on screen only: what is exported is the whole model. Drag any of the five marks on the \
             frame to slide the plane.",
        ))
        .selectable(false),
    );
}

/// The buttons along the foot: put the plane away, or stand it back in the
/// middle of the model.
pub(crate) fn actions(app: &mut App, ui: &mut egui::Ui) {
    if ui::dialog_button(ui, "Done", true).clicked() {
        app.run(Command::ToggleSection);
    }
    // At the other end of the row, the way the measure tool's Clear is: a plane
    // swept out past the model shows an uncut shape and no sign of what to do
    // about it, and orbiting round to find the frame again is the long way back.
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        if ui
            .add(egui::Button::new("Back to the middle"))
            .on_hover_text("Stand the plane in the middle of the model again")
            .clicked()
        {
            let middle = middle_of(app.evaluated.mesh.bounds(), app.scene.settings.section.axis());
            app.set_section_offset(middle);
        }
    });
}

/// What the plane is doing, for the status line: where it stands and which side
/// of it is being kept.
pub fn readout(app: &App) -> String {
    let section = app.scene.settings.section;
    let unit = app.unit();
    format!(
        "Section at {} = {} {}, keeping what is {} it",
        section.axis_label(),
        format_length(section.offset, unit),
        unit.suffix(),
        // Said as the coordinate rather than as the camera sees it: which side
        // is the near one depends on where the model has been orbited to, and
        // the plane does not move when it is.
        if section.flipped { "above" } else { "below" }
    )
}

/// Put the plane back in the middle of the model, along the axis it is on.
///
/// What "on" means for a section that has just been switched on: an offset of
/// zero cuts nothing at all for a part that does not straddle the origin, and a
/// section that appears to do nothing reads as a broken one.
pub fn middle_of(bounds: Option<(Vec3, Vec3)>, axis: usize) -> f64 {
    match bounds {
        Some((lo, hi)) => (component(lo, axis) + component(hi, axis)) * 0.5,
        None => 0.0,
    }
}
