//! The panel's list of constraints, one row each as the workplane list's: its name, red when it
//! repeats or fights another, a dimension's value typed or scrubbed at the right, and a bin to drop
//! it. A click on the row selects it.

use super::*;
use crate::app::{App, Status};
use crate::icon::{self, Glyph};
use crate::popup;
use crate::theme::{self, metric, token};
use simple3d_core::primitive::ParamKind;

/// A value field's width: a length with its unit.
const FIELD_WIDTH: f32 = 84.0;
/// The bin's side, as the workplane rows'.
const ICON: f32 = 16.0;

/// Lengths are never negative; an angle keeps its turn and is typed as a size.
const LENGTH: ParamKind = ParamKind::Length { min: 0.0 };
const ANGLE: ParamKind = ParamKind::Angle { min: 0.0, max: 180.0, wrap: false };

pub(crate) fn list(app: &mut App, ui: &mut egui::Ui, tool: &mut SketchTool) {
    let count = tool.sketch.constraints.len();
    let header = format!("Constraints ({count})");
    super::window::group(ui, &header, |ui| {
        if count == 0 {
            let hint = "Select points or curves and use the Constrain buttons, or the Dimension tool.";
            ui.add(egui::Label::new(theme::hint(hint)).selectable(false));
            return;
        }
        let mut dropped = None;
        let mut retyped = None;
        ui.spacing_mut().item_spacing.y = 0.0;
        for constraint in tool.sketch.constraints.clone() {
            let pick = Pick::Constraint(constraint.id);
            let marked = tool.conflicts.contains(&constraint.id) || tool.analysis.redundant.contains(&constraint.id);
            let selected = tool.selection.contains(&pick);
            let row = Row { id: constraint.id, name: constraint.kind.label(), marked, selected };
            let asked = show_row(ui, row, |ui| {
                let value = constraint.kind.value()?;
                let mut shown = value.abs();
                let name = format!("sketch-constraint-{}", constraint.id);
                let kind = if constraint.kind.is_angle() { ANGLE } else { LENGTH };
                popup::number(app, ui, ("sketch-constraint", &name), FIELD_WIDTH, kind, &mut shown);
                ((shown - value.abs()).abs() > 1e-12).then_some(if value < 0.0 { -shown } else { shown })
            });
            match asked {
                Asked::Select => tool.selection = vec![pick],
                Asked::Drop => dropped = Some(constraint.id),
                Asked::Retype(value) => retyped = Some((constraint.id, value)),
                Asked::Nothing => {}
            }
        }
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

/// One constraint's row as drawn.
struct Row {
    id: ConstraintId,
    name: &'static str,
    marked: bool,
    selected: bool,
}

/// What a row was asked to do this frame.
enum Asked {
    Nothing,
    Select,
    Drop,
    Retype(f64),
}

/// The row: marked as the outliner marks a selected row, the name at the left, then `value` (the
/// dimension's field, handing back a typed value) and the bin at the right.
fn show_row(ui: &mut egui::Ui, row: Row, value: impl FnOnce(&mut egui::Ui) -> Option<f64>) -> Asked {
    let height = metric::INPUT_ROW + 2.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::hover());
    let id = egui::Id::new(("sketch-constraint-row", row.id));
    let response = ui.interact(rect, id, egui::Sense::click());
    let painter = ui.painter();
    if row.selected {
        painter.rect_filled(rect, 0.0, token::ACCENT.gamma_multiply(0.13));
        let bar = egui::Rect::from_min_size(rect.left_top(), egui::vec2(2.0, rect.height()));
        painter.rect_filled(bar, 0.0, token::ACCENT.gamma_multiply(0.5));
    } else if response.hovered() {
        painter.rect_filled(rect, 0.0, token::SURFACE_3);
    }
    let colour = if row.marked { token::DANGER } else { token::TEXT_HI };
    painter.text(
        egui::pos2(rect.left() + 6.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        row.name,
        egui::FontId::proportional(theme::font::VALUE),
        colour,
    );
    let mut asked = if response.clicked() { Asked::Select } else { Asked::Nothing };
    let bin = egui::Rect::from_center_size(egui::pos2(rect.right() - 12.0, rect.center().y), egui::Vec2::splat(ICON));
    let drop = ui.interact(bin, id.with("drop"), egui::Sense::click()).on_hover_text("Remove this constraint");
    drop.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Remove this constraint"));
    let ink = if drop.hovered() { token::DANGER } else { token::TEXT_LO };
    icon::draw(ui.painter(), bin.shrink(1.0), Glyph::Delete, ink);
    if drop.clicked() {
        asked = Asked::Drop;
    }
    let field = egui::Rect::from_min_max(
        egui::pos2(rect.left(), rect.top() + 1.0),
        egui::pos2(bin.left() - 6.0, rect.bottom() - 1.0),
    );
    let mut child =
        ui.new_child(egui::UiBuilder::new().max_rect(field).layout(egui::Layout::right_to_left(egui::Align::Center)));
    if let Some(typed) = value(&mut child) {
        asked = Asked::Retype(typed);
    }
    asked
}
