//! The settings for lining up and for spreading along an axis.

use super::*;
use crate::app::App;
use crate::{popup, theme};
use simple3d_core::primitive::ParamKind;

/// Per axis, which side lines up; and what they line up on.
pub(super) fn align_controls(app: &mut App, ui: &mut egui::Ui, tool: &mut ArrangeTool) {
    egui::Grid::new("arrange-align").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        for axis in 0..3 {
            theme::axis_chip(ui, ui.id().with(("arrange-axis", axis)), axis);
            ui.horizontal(|ui| {
                if theme::choice(ui, tool.align[axis].is_none(), "Leave")
                    .on_hover_text("Do not move along this axis")
                    .clicked()
                {
                    tool.align[axis] = None;
                }
                for side in Side::ALL {
                    if theme::choice(ui, tool.align[axis] == Some(side), side.label()).clicked() {
                        tool.align[axis] = Some(side);
                    }
                }
            });
            ui.end_row();
        }
        popup::label(ui, "Relative to", "What the objects line up on.");
        ui.horizontal_wrapped(|ui| {
            if theme::choice(ui, !tool.to_key, "All of them")
                .on_hover_text("The box around every object: aligning by the minimum lines them up on the lowest one.")
                .clicked()
            {
                tool.to_key = false;
            }
            // Only one of the objects being lined up can be held still, not a group opened on for its contents.
            let arranged = tool.arranged(&app.scene);
            let key = tool
                .key
                .filter(|id| arranged.contains(id))
                .and_then(|id| app.scene.get(id))
                .map(|node| node.name.clone());
            if key.is_none() {
                tool.to_key = false;
            }
            let named = key.as_deref().map_or("Last selected".to_string(), |name| format!("Last selected ({name})"));
            let response = ui.add_enabled_ui(key.is_some(), |ui| theme::choice(ui, tool.to_key, &named)).inner;
            if response.on_hover_text("Hold the last selected object still and line the others up on it.").clicked() {
                tool.to_key = true;
            }
        });
        ui.end_row();
    });
}

/// The axis, and how the objects are spaced along it.
pub(super) fn distribute_controls(app: &mut App, ui: &mut egui::Ui, tool: &mut ArrangeTool) {
    egui::Grid::new("arrange-distribute").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        popup::label(ui, "Along", "The axis the objects are spread along.");
        ui.horizontal(|ui| {
            for (axis, name) in [(0, "X"), (1, "Y"), (2, "Z")] {
                if theme::choice(ui, tool.axis == axis, name).clicked() {
                    tool.axis = axis;
                }
            }
        });
        ui.end_row();

        popup::label(ui, "Spacing", "What is made even.");
        ui.horizontal_wrapped(|ui| {
            for (spacing, name, hover) in [
                (Spacing::Gaps, "Equal gaps", "The outer two stay; the space between neighbours is the same."),
                (Spacing::Centres, "Equal centres", "The outer two stay; the centres are the same distance apart."),
                (Spacing::Fixed, "Set gap", "The first stays; each next one follows the gap below after the last."),
            ] {
                if theme::choice(ui, tool.spacing == spacing, name).on_hover_text(hover).clicked() {
                    tool.spacing = spacing;
                }
            }
        });
        ui.end_row();

        if tool.spacing == Spacing::Fixed {
            popup::label(ui, &format!("Gap ({})", app.unit().suffix()), "The space left between neighbours.");
            number(app, ui, "gap", ParamKind::Length { min: 0.0 }, &mut tool.gap);
            ui.end_row();
        }
    });
}

/// A drag-or-type number scoped to this tool, so a drag passed between tool windows cannot edit it.
pub(super) fn number(app: &mut App, ui: &mut egui::Ui, name: &str, kind: ParamKind, value: &mut f64) {
    popup::number(app, ui, ("arrange-field", &format!("arrange-{name}")), FIELD_WIDTH, kind, value);
}
