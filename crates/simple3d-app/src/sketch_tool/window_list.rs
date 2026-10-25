//! The window's list of constraints: each one named, a dimension's value typed or scrubbed in place,
//! and a cross to drop it.

use super::*;
use crate::app::{App, Status};
use crate::popup;
use simple3d_core::primitive::ParamKind;

/// A value field's width: a length with its unit.
const FIELD_WIDTH: f32 = 90.0;

/// Lengths are never negative; an angle keeps its turn and is typed as a size.
const LENGTH: ParamKind = ParamKind::Length { min: 0.0 };
const ANGLE: ParamKind = ParamKind::Angle { min: 0.0, max: 180.0, wrap: false };

pub(crate) fn list(app: &mut App, ui: &mut egui::Ui, tool: &mut SketchTool) {
    let count = tool.sketch.constraints.len();
    let header = format!("Constraints ({count})");
    egui::CollapsingHeader::new(header).id_salt("sketch-constraints").default_open(true).show(ui, |ui| {
        if count == 0 {
            ui.add(
                egui::Label::new(crate::theme::hint("Select points or curves and use the buttons above."))
                    .selectable(false),
            );
            return;
        }
        let mut dropped = None;
        let mut retyped = None;
        egui::Grid::new("sketch-constraint-list").num_columns(3).spacing([8.0, 4.0]).show(ui, |ui| {
            for constraint in tool.sketch.constraints.clone() {
                let pick = Pick::Constraint(constraint.id);
                let marked =
                    tool.conflicts.contains(&constraint.id) || tool.analysis.redundant.contains(&constraint.id);
                let text = egui::RichText::new(constraint.kind.label());
                let text = if marked { text.color(crate::theme::token::DANGER) } else { text };
                if ui.add(egui::Button::selectable(tool.selection.contains(&pick), text)).clicked() {
                    tool.selection = vec![pick];
                }
                match constraint.kind.value() {
                    Some(value) => {
                        let angle = constraint.kind.is_angle();
                        let mut shown = value.abs();
                        let name = format!("sketch-constraint-{}", constraint.id);
                        let kind = if angle { ANGLE } else { LENGTH };
                        popup::number(app, ui, ("sketch-constraint", &name), FIELD_WIDTH, kind, &mut shown);
                        if (shown - value.abs()).abs() > 1e-12 {
                            retyped = Some((constraint.id, if value < 0.0 { -shown } else { shown }));
                        }
                    }
                    None => {
                        ui.label("");
                    }
                }
                if popup::drop_button(ui, "Remove this constraint") {
                    dropped = Some(constraint.id);
                }
                ui.end_row();
            }
        });
        if let Some(id) = dropped {
            tool.change(|sketch| sketch.remove_constraint(id));
        }
        if let Some((id, value)) = retyped {
            let before = tool.sketch.clone();
            match tool.sketch.set_value(id, value) {
                Ok(()) => {
                    tool.remember(before);
                    tool.refresh();
                }
                Err(refusal) => app.status = Status::Warning(refusal.message()),
            }
        }
    });
}
