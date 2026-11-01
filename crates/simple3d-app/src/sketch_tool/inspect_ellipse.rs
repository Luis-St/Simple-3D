//! The popup's rows for an ellipse or elliptical arc: how long and how wide it is, and its centre.

use super::inspect_fields::{place_rows, resize_circle, LENGTH};
use super::inspect_widgets::number;
use super::shapes::gap;
use super::*;
use crate::app::{App, Status};

/// Its half-length, moving the end of its major axis along the axis, its half-width, set straight,
/// and the place of its centre.
pub(super) fn rows(
    app: &mut App,
    ui: &mut egui::Ui,
    tool: &mut SketchTool,
    id: CurveId,
    (center, major): (PointId, PointId),
    minor: f64,
) -> Option<Status> {
    let (c, m) = (tool.sketch.at(center), tool.sketch.at(major));
    let half = gap(c, m);
    let mut message = None;
    let field = format!("half-length-{id}");
    if let Some(value) = number(app, ui, (&field, "Half length"), LENGTH, half, tool.all_fixed(&[major])) {
        let d = [(m[0] - c[0]) / half.max(1e-12), (m[1] - c[1]) / half.max(1e-12)];
        message = tool.move_points(&field, &[(major, [c[0] + d[0] * value, c[1] + d[1] * value])]);
    }
    ui.end_row();
    let field = format!("half-width-{id}");
    if let Some(value) = number(app, ui, (&field, "Half width"), LENGTH, minor, false) {
        message = resize_circle(tool, &field, id, value);
    }
    ui.end_row();
    place_rows(app, ui, tool, center, "Centre").or(message)
}
