//! Dragging a dimension's label to where it reads best, as on a drawing; the dimension lines follow.

use super::*;
use crate::view::View;

/// Start, carry on or end dragging the label under the pointer; whether the pointer was taken.
pub(crate) fn drag_label(tool: &mut SketchTool, response: &egui::Response, view: &View, cursor: egui::Pos2) -> bool {
    if tool.label_drag.is_none() && response.drag_started_by(egui::PointerButton::Primary) {
        if let Some(Pick::Constraint(id)) = tool.hover {
            if tool.sketch.constraint(id).is_some_and(|c| c.kind.value().is_some()) {
                tool.label_drag = Some((id, tool.sketch.clone()));
                tool.selection = vec![Pick::Constraint(id)];
            }
        }
    }
    let Some((id, _)) = tool.label_drag else { return false };
    if let Some(at) = tool.under(view, cursor) {
        let from = tool.sketch.constraint(id).and_then(|c| super::dimension::anchor(&tool.sketch, &c.kind));
        if let (Some(from), Some(constraint)) = (from, tool.sketch.constraint_mut(id)) {
            constraint.label = Some([at[0] - from[0], at[1] - from[1]]);
        }
    }
    if !response.dragged() {
        let (_, before) = tool.label_drag.take().expect("checked above");
        if tool.sketch != before {
            tool.remember(before);
            tool.refresh();
        }
    }
    true
}
