//! The tool's window: the workplane's name, where it sits on its face, its turn and spacing, and the
//! buttons.

use super::*;
use crate::app::App;
use crate::popup::{self, PopupEvent, PopupSpec};
use crate::{theme, ui};
use simple3d_core::primitive::ParamKind;

/// A number field's width: enough for a length with its unit.
const FIELD_WIDTH: f32 = 90.0;

/// A slide or lift, either way.
const OFFSET: ParamKind = ParamKind::Length { min: f64::NEG_INFINITY };

/// A turn about the normal, read as a direction.
const SPIN: ParamKind = ParamKind::Angle { min: 0.0, max: 360.0, wrap: true };

/// The grid spacing: never zero, which would draw no grid.
const SPACING: ParamKind = ParamKind::Length { min: 0.01 };

/// The tool's window over the viewport, while open (issue 125).
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    let Some(tool) = app.workplane_tool.as_ref() else { return };
    let title = if tool.editing.is_some() { "Change workplane" } else { "Add a workplane" };
    let spec = PopupSpec { key: KEY, title, width: WIDTH };
    if app.tool_popup(ctx, spec, body, actions) == PopupEvent::Closed {
        app.close_workplane_tool();
    }
}

/// The settings, under a line saying what to do next. The tool is lifted out of the app while
/// drawing, since the fields also borrow app state.
fn body(app: &mut App, ui: &mut egui::Ui) {
    let Some(mut tool) = app.workplane_tool.take() else { return };
    let hint = match tool.picked {
        false => "Click the face to put the workplane on in the viewport.",
        true => "Click another face to move it there; what is set here is kept.",
    };
    ui.add(egui::Label::new(theme::hint(hint)).selectable(false));
    ui.add_space(4.0);
    rows(app, ui, &mut tool.plane);
    app.workplane_tool = Some(tool);
}

fn rows(app: &mut App, ui: &mut egui::Ui, plane: &mut Workplane) {
    let unit = app.unit().suffix();
    egui::Grid::new("workplane-tool").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        popup::label(ui, "Name", "What the workplane is called in the Workplanes panel.");
        ui.add(egui::TextEdit::singleline(&mut plane.name).desired_width(2.0 * FIELD_WIDTH));
        ui.end_row();
        for (axis, name, hover) in [
            (0, "X", "Slide it along its own X, the face's level direction."),
            (1, "Y", "Slide it along its own Y, across the face."),
            (2, "Z", "Lift it off the face along its normal; below zero sinks it in."),
        ] {
            popup::label(ui, &format!("{name} ({unit})"), hover);
            let field = ("workplane-field", ["workplane-x", "workplane-y", "workplane-z"][axis]);
            popup::number(app, ui, field, FIELD_WIDTH, OFFSET, &mut plane.offset[axis]);
            ui.end_row();
        }
        popup::label(ui, "Rotation (deg)", "Turn its grid about the face's normal.");
        popup::number(app, ui, ("workplane-field", "workplane-spin"), FIELD_WIDTH, SPIN, &mut plane.spin);
        ui.end_row();
        popup::label(ui, &format!("Spacing ({unit})"), "How far apart its grid lines are.");
        popup::number(app, ui, ("workplane-field", "workplane-spacing"), FIELD_WIDTH, SPACING, &mut plane.spacing);
        ui.end_row();
    });
}

fn actions(app: &mut App, ui: &mut egui::Ui) {
    let Some(tool) = app.workplane_tool.as_ref() else { return };
    let label = if tool.editing.is_some() { "Apply" } else { "Place" };
    if ui::dialog_button(ui, label, tool.picked).clicked() {
        app.apply_workplane_tool();
    }
    crate::app_chrome::cancel_at_left(ui, |ui| {
        if ui::dialog_button(ui, "Cancel", true).clicked() {
            app.close_workplane_tool();
        }
    });
}
