//! The tool's popup: the profile, the axis, the angle and its direction, with the solid shown live.

use super::*;
use crate::app::{App, Status};
use crate::popup::{self, PopupEvent, PopupSpec};
use crate::theme;
use crate::ui;
use simple3d_core::keymap::Command;
use simple3d_core::primitive::ParamKind;

/// Scopes the window's fields, so a drag passed between tool windows cannot edit the wrong number.
const SCOPE: &str = "revolve-field";

/// A number field's width: enough for an angle with its unit.
const FIELD_WIDTH: f32 = 90.0;

/// The tool's window over the viewport, while the tool is on the rail.
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    if app.mode != crate::gizmo::Mode::Revolve {
        return;
    }
    let spec = PopupSpec { key: KEY, title: "Revolve", width: WIDTH };
    let event = app.tool_popup(ctx, spec, body, actions);
    if event == PopupEvent::Closed {
        app.run(Command::ModeMove);
    }
    app.revolve.refresh();
}

fn body(app: &mut App, ui: &mut egui::Ui) {
    let mut tool = std::mem::take(&mut app.revolve);
    egui::Grid::new("revolve-grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        popup::label(ui, "Profile", "Click a closed region of a sketch or a flat face in the viewport.");
        ui.horizontal(|ui| {
            let text = match &tool.profile {
                Some(Target::Region(..)) => "Sketch region",
                Some(Target::Face(_)) => "Face",
                None => "None",
            };
            ui.add(egui::Label::new(theme::value(text)).selectable(false));
            if ui.add_enabled(tool.profile.is_some(), egui::Button::new("Clear")).clicked() {
                tool.clear();
            }
        });
        ui.end_row();

        popup::label(
            ui,
            "Axis",
            "The line to turn about. Choose an axis of the world, which is taken in the profile's plane when \
             it runs parallel to it, or click a line of the sketch or a straight edge in the viewport.",
        );
        ui.horizontal(|ui| {
            for (k, name) in ["X", "Y", "Z"].into_iter().enumerate() {
                let chosen = tool.axis.as_ref().is_some_and(|axis| axis.source == AxisSource::World(k));
                let fits = tool.profile.as_ref().map(|profile| axis::world(profile, k));
                let enabled = matches!(fits, Some(Ok(_)));
                let response = ui.add_enabled_ui(enabled, |ui| theme::choice(ui, chosen, name)).inner;
                let response = match &fits {
                    Some(Err(why)) => response.on_disabled_hover_text(*why),
                    None => response.on_disabled_hover_text("Pick a profile first"),
                    Some(Ok(_)) => response.on_hover_text(format!("Turn about the world's {name} axis")),
                };
                if response.clicked() {
                    if let Some(Ok(axis)) = fits {
                        tool.set_axis(axis);
                    }
                }
            }
            let picked = tool.axis.as_ref().is_some_and(|axis| axis.source == AxisSource::Picked);
            let text = if picked { "Picked line" } else { "or click a line" };
            ui.add(egui::Label::new(if picked { theme::value(text) } else { theme::hint(text) }).selectable(false));
        });
        ui.end_row();

        popup::label(
            ui,
            "Angle (deg)",
            "How far the profile turns about the axis; 360 makes a whole solid of revolution.",
        );
        ui.horizontal(|ui| {
            let kind = ParamKind::Angle { min: 1.0, max: 360.0, wrap: false };
            popup::number(app, ui, (SCOPE, "angle"), FIELD_WIDTH, kind, &mut tool.angle);
            tool.angle = tool.angle.clamp(1.0, 360.0);
            for preset in [90.0, 180.0, 360.0] {
                if theme::choice(ui, tool.angle == preset, &format!("{preset}")).clicked() {
                    tool.angle = preset;
                }
            }
        });
        ui.end_row();

        popup::label(ui, "Reverse", "Turn the other way about the axis.");
        ui.checkbox(&mut tool.flip, "");
        ui.end_row();

        popup::label(
            ui,
            "Result",
            "Turned out of the profile's front it adds; turned into the object the profile lies on, it cuts.",
        );
        let result = if tool.axis.is_none() {
            "-"
        } else if tool.cuts() {
            "Cut"
        } else {
            "Add"
        };
        ui.add(egui::Label::new(theme::value(result)).selectable(false));
        ui.end_row();
    });
    let note = match &tool.preview {
        Some((_, Err(why))) => Some(why.clone()),
        _ if tool.axis.is_none() => Some(step_hint(&tool).trim_start_matches("Revolve: ").to_string()),
        _ => None,
    };
    if let Some(note) = note {
        ui.add_space(4.0);
        ui.add(egui::Label::new(theme::hint(note)).selectable(false));
    }
    app.revolve = tool;
}

fn actions(app: &mut App, ui: &mut egui::Ui) {
    let ready = app.revolve.preview.as_ref().is_some_and(|(_, made)| made.is_ok());
    if ui::dialog_button(ui, "Apply", ready).clicked() {
        super::commit::commit(app);
    }
    crate::app_chrome::cancel_at_left(ui, |ui| {
        if ui::dialog_button(ui, "Cancel", true).clicked() {
            if app.revolve.profile.is_some() {
                app.revolve.clear();
                app.status = Status::Info(step_hint(&app.revolve).into());
            } else {
                app.run(Command::ModeMove);
            }
        }
    });
}
