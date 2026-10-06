//! The tool's window: the treatment and its numbers, then what is picked.

use super::*;
use crate::popup::{self, PopupEvent, PopupSpec};
use crate::theme;
use crate::ui;
use simple3d_core::primitive::ParamKind;

/// Scopes the window's fields, so a drag passed between tool windows cannot edit the wrong number.
const SCOPE: &str = "round-field";

/// A number field's width: enough for a length with its unit.
const FIELD_WIDTH: f32 = 90.0;

/// The tool's window over the viewport, while open.
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    if app.round_tool.is_none() {
        return;
    }
    let spec = PopupSpec { key: KEY, title: "Round and bevel", width: WIDTH };
    let event = app.tool_popup(ctx, spec, body, actions);
    if event == PopupEvent::Closed && app.round_tool.is_some() {
        app.open_round_tool();
    }
    // The viewport's picks and this window's numbers, shown in the model.
    app.sync_round_draft();
}

/// The treatment and its numbers, then the picks. The tool is lifted out of the app while drawing, since
/// the fields also borrow app state.
fn body(app: &mut App, ui: &mut egui::Ui) {
    let Some(mut tool) = app.round_tool.take() else { return };
    let length = format!("({})", app.unit().suffix());
    let (edges, corners) = (tool.edges.len(), tool.corners.len() + tool.joints.len());
    egui::Grid::new("round-grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        popup::label(ui, "Type", "Round leaves an arc between the faces; chamfer a flat cut.");
        ui.horizontal(|ui| {
            for kind in Kind::ALL {
                if theme::choice(ui, tool.kind == kind, kind.label()).clicked() {
                    tool.kind = kind;
                }
            }
        });
        ui.end_row();

        match tool.kind {
            Kind::Round => {
                popup::label(
                    ui,
                    &format!("Radius {length}"),
                    "The arc's radius. It meets each face this far from the edge on a square edge, further on a \
                     sharper one.",
                );
                popup::number(
                    app,
                    ui,
                    (SCOPE, "radius"),
                    FIELD_WIDTH,
                    ParamKind::Length { min: 0.01 },
                    &mut tool.radius,
                );
                ui.end_row();
                popup::label(ui, "Segments", "How many flat pieces each arc is made of.");
                let mut segments = f64::from(tool.segments);
                popup::number(
                    app,
                    ui,
                    (SCOPE, "segments"),
                    FIELD_WIDTH,
                    ParamKind::Count { min: 1, max: 64 },
                    &mut segments,
                );
                tool.segments = segments.round().clamp(1.0, 64.0) as u32;
                ui.end_row();
            }
            Kind::Chamfer => {
                popup::label(ui, &format!("Distance {length}"), "How far back along each face the cut starts.");
                popup::number(
                    app,
                    ui,
                    (SCOPE, "distance"),
                    FIELD_WIDTH,
                    ParamKind::Length { min: 0.01 },
                    &mut tool.distance,
                );
                ui.end_row();
            }
        }

        popup::label(
            ui,
            "Blend corners",
            "Also treat every corner whose edges are all picked, so three rounded edges meet in a ball \
             rather than a point.",
        );
        ui.checkbox(&mut tool.blend_corners, "");
        ui.end_row();

        popup::label(
            ui,
            "Merge edges",
            "Pick an edge running straight on from one object into the next as one edge. Off, each \
             object's stretch of it is picked on its own.",
        );
        ui.checkbox(&mut tool.merge_edges, "");
        ui.end_row();

        popup::label(
            ui,
            "Extend edges",
            "Treat each picked edge on to where the body ends, as if an earlier bevel or round had not cut \
             its end off at a corner.",
        );
        ui.checkbox(&mut tool.extend_edges, "");
        ui.end_row();

        popup::label(
            ui,
            "Picked",
            "Click an edge or a corner in the viewport to pick it, again to drop it; drag an edge to size it.",
        );
        ui.horizontal(|ui| {
            let text = format!(
                "{edges} edge{}, {corners} corner{}",
                if edges == 1 { "" } else { "s" },
                if corners == 1 { "" } else { "s" }
            );
            ui.add(egui::Label::new(theme::value(text)).selectable(false));
            if ui.add_enabled(!tool.is_empty(), egui::Button::new("Clear")).clicked() {
                tool.edges.clear();
                tool.corners.clear();
                tool.joints.clear();
            }
        });
        ui.end_row();
    });
    let too_small = tool.too_small();
    if too_small > 0 {
        ui.add_space(4.0);
        let text = format!("{too_small} too small, left as they are");
        ui.add(egui::Label::new(theme::hint(text)).selectable(false));
    }
    if tool.is_empty() {
        ui.add_space(4.0);
        ui.add(
            egui::Label::new(theme::hint("Click edges and corners to pick them, drag an edge to size it."))
                .selectable(false),
        );
    }
    app.round_tool = Some(tool);
}

fn actions(app: &mut App, ui: &mut egui::Ui) {
    let ready = app.round_tool.as_ref().is_some_and(|tool| !tool.is_empty());
    if ui::dialog_button(ui, "Apply", ready).clicked() {
        app.apply_round_tool();
    }
    crate::app_chrome::cancel_at_left(ui, |ui| {
        if ui::dialog_button(ui, "Cancel", true).clicked() {
            app.cancel_round_tool();
        }
    });
}
