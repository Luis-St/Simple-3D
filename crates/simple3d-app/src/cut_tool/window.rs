//! The tool's window: the face, how far the cut reaches, what Cut comes to, and its buttons.

use super::*;
use crate::app::App;
use crate::popup::{self, PopupEvent, PopupSpec};
use crate::{theme, ui};
use simple3d_core::primitive::ParamKind;

/// A number field's width: enough for a length with its unit.
const FIELD_WIDTH: f32 = 90.0;

/// The tool's window over the viewport, while open (issue 130).
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    if app.cut_tool.is_none() {
        return;
    }
    let spec = PopupSpec { key: KEY, title: "Cut on a face", width: WIDTH };
    if app.tool_popup(ctx, spec, body, actions) == PopupEvent::Closed {
        app.close_cut_tool();
    }
}

/// The settings, then one line saying what Cut comes to or what is still to do. The tool is lifted out
/// of the app while drawing, since the fields also borrow app state.
fn body(app: &mut App, ui: &mut egui::Ui) {
    let Some(mut tool) = app.cut_tool.take() else { return };
    rows(app, ui, &mut tool);
    app.cut_tool = Some(tool);
    ui.add_space(4.0);
    summary(app, ui);
}

fn rows(app: &mut App, ui: &mut egui::Ui, tool: &mut CutTool) {
    let hint = match tool.face {
        None => "Click the face to cut along in the viewport.",
        Some(_) => "Click another face to change it; a right-click lets it go.",
    };
    ui.add(egui::Label::new(theme::hint(hint)).selectable(false));
    ui.add_space(4.0);
    egui::Grid::new("cut-tool").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        popup::label(ui, "Reach", "How far along the face's plane the cut goes.");
        ui.horizontal(|ui| {
            for (limited, name, hover) in [
                (false, "Endless", "Cut every object the face's plane runs through."),
                (true, "Limited area", "Cut only objects whose whole cut lies in an area round the face's middle."),
            ] {
                if theme::choice(ui, tool.limited == limited, name).on_hover_text(hover).clicked() {
                    tool.limited = limited;
                }
            }
        });
        ui.end_row();
        if tool.limited {
            let unit = app.unit().suffix();
            let length = ParamKind::Length { min: 0.0 };
            for (axis, name, hover) in [
                (0, "Width", "The area's side along the plane's level direction."),
                (1, "Height", "The area's side square to its width."),
            ] {
                popup::label(ui, &format!("{name} ({unit})"), hover);
                let field = ("cut-field", if axis == 0 { "cut-width" } else { "cut-height" });
                popup::number(app, ui, field, FIELD_WIDTH, length, &mut tool.size[axis]);
                ui.end_row();
            }
        }
    });
}

/// What Cut would do, or the next step.
fn summary(app: &mut App, ui: &mut egui::Ui) {
    let Some(tool) = app.cut_tool.as_ref() else { return };
    let (face, limited) = (tool.face, tool.limited);
    let text = match face {
        None => "Nothing is cut until a face is picked.".to_string(),
        Some(_) => match app.cut_planned().len() {
            0 if limited => "No object's cut lies wholly within the area.".to_string(),
            0 => "The face's plane runs through no object.".to_string(),
            1 => "1 object is cut in two where the plane shows.".to_string(),
            count => format!("{count} objects are cut in two where the plane shows."),
        },
    };
    ui.add(egui::Label::new(theme::hint(text)).selectable(false));
}

fn actions(app: &mut App, ui: &mut egui::Ui) {
    let ready = app.cut_tool.as_ref().is_some_and(|tool| tool.face.is_some())
        && app.cut_job.is_none()
        && !app.cut_planned().is_empty();
    if ui::dialog_button(ui, "Cut", ready).clicked() {
        app.start_cut();
    }
    crate::app_chrome::cancel_at_left(ui, |ui| {
        if ui::dialog_button(ui, "Cancel", true).clicked() {
            app.close_cut_tool();
        }
    });
}
