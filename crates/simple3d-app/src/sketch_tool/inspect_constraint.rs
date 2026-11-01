//! The popup's rows for a constraint: what it holds, and for a dimension its value and whether it
//! drives the sketch or only measures it.

use super::inspect_fields::LENGTH;
use super::inspect_widgets::{number, switch};
use super::*;
use crate::app::{App, Status};
use crate::theme;
use simple3d_core::primitive::ParamKind;

const ANGLE: ParamKind = ParamKind::Angle { min: 0.0, max: 180.0, wrap: false };

/// The id of dimension `id`'s value field in the popup, to open it for typing.
pub(crate) fn value_field(id: ConstraintId) -> egui::Id {
    egui::Id::new(("sketch-inspect", format!("value-{id}")))
}

pub(crate) fn constraint(app: &mut App, ui: &mut egui::Ui, tool: &mut SketchTool, id: ConstraintId) -> Option<Status> {
    let constraint = tool.sketch.constraint(id)?.clone();
    let names = super::window_list::names(&tool.sketch, &constraint.kind);
    ui.add(egui::Label::new(theme::hint(format!("On {names}"))).selectable(false));
    let Some(value) = constraint.kind.value() else {
        let hint = "Holds while it is there; Delete drops it.";
        ui.add(egui::Label::new(theme::hint(hint)).selectable(false));
        return None;
    };
    let mut message = None;
    let (label, kind) = if constraint.kind.is_angle() { ("Angle", ANGLE) } else { ("Value", LENGTH) };
    if constraint.driven {
        let said = super::dimension_draw::text(&constraint.kind, value, false, tool.unit);
        ui.add(egui::Label::new(theme::value(format!("{label}: {said}"))).selectable(false));
    } else {
        egui::Grid::new(("sketch-inspect-constraint", id)).num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            let field = format!("value-{id}");
            if let Some(shown) = number(app, ui, (&field, label), kind, value.abs(), false) {
                message = tool.retype(&field, id, if value < 0.0 { -shown } else { shown });
            }
            ui.end_row();
        });
    }
    let hover = "A reference dimension only measures: it follows the sketch instead of deciding it.";
    if switch(ui, constraint.driven, "Reference", hover) {
        let before = tool.sketch.clone();
        match tool.sketch.set_driven(id, !constraint.driven) {
            Ok(()) => {
                tool.remember(before);
                tool.refresh();
            }
            Err(refusal) => {
                if let simple3d_sketch::solver::Refusal::Redundant(others) = &refusal {
                    tool.conflicts = others.clone();
                }
                message = Some(Status::Warning(refusal.message()));
            }
        }
    }
    message
}
