//! The popup's rows for each kind of thing: a line's length, angle and ends, a round curve's radius
//! and centre, a point's place, a dimension's value, and the switches under them.

use super::inspect_widgets::{lock, number, switch};
use super::shapes::{angle_of, gap};
use super::*;
use crate::app::{App, Status};
use crate::panel_properties::POINT;
use crate::theme;
use simple3d_core::primitive::ParamKind;
use simple3d_sketch::{ConstraintKind, Shape};

pub(super) const LENGTH: ParamKind = ParamKind::Length { min: 0.0 };
const DIRECTION: ParamKind = ParamKind::Angle { min: 0.0, max: 360.0, wrap: true };

pub(crate) fn curve(app: &mut App, ui: &mut egui::Ui, tool: &mut SketchTool, id: CurveId) -> Option<Status> {
    let curve = tool.sketch.curve(id)?.clone();
    let mut message = None;
    if curve.reference {
        let hint = "Copied from an edge of the model, so it stays where the model has it.";
        ui.add(egui::Label::new(theme::hint(hint)).selectable(false));
    } else {
        egui::Grid::new(("sketch-inspect-grid", id)).num_columns(3).spacing([8.0, 4.0]).show(ui, |ui| {
            message = match curve.shape {
                Shape::Line { a, b } => line_rows(app, ui, tool, id, (a, b)),
                Shape::Circle { center, radius } => round_rows(app, ui, tool, id, center, radius, None),
                Shape::Arc { center, start, end } => {
                    let radius = gap(tool.sketch.at(center), tool.sketch.at(start));
                    round_rows(app, ui, tool, id, center, radius, Some((start, end)))
                }
                Shape::Ellipse { center, major, minor } | Shape::EllipseArc { center, major, minor, .. } => {
                    super::inspect_ellipse::rows(app, ui, tool, id, (center, major), minor)
                }
                _ => None,
            };
        });
    }
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        if curve.shape.is_line() && !curve.reference {
            for vertical in [false, true] {
                let existing = tool.holds(|k| match *k {
                    ConstraintKind::Horizontal { line } => !vertical && line == id,
                    ConstraintKind::Vertical { line } => vertical && line == id,
                    _ => false,
                });
                let (label, kind) = match vertical {
                    false => ("Horizontal", ConstraintKind::Horizontal { line: id }),
                    true => ("Vertical", ConstraintKind::Vertical { line: id }),
                };
                if switch(ui, existing.is_some(), label, "Keep the line square to the sketch's axes") {
                    message = tool.toggle(existing, kind).or(message.take());
                }
            }
        }
        let hover = "Helper geometry is constrained against and can be an axis, but bounds no region.";
        if switch(ui, curve.construction, "Helper", hover) {
            tool.change(|sketch| {
                if let Some(curve) = sketch.curve_mut(id) {
                    curve.construction = !curve.construction;
                }
            });
        }
        let points = curve.shape.points();
        if !curve.reference && switch(ui, tool.all_fixed(&points), "Fixed", "Hold all its points where they are") {
            tool.toggle_fix(&points);
        }
        if let Shape::Spline { points, closed } | Shape::Bspline { points, closed } = &curve.shape {
            if points.len() >= 3 && switch(ui, *closed, "Closed", "Run the spline back round to its first point") {
                tool.change(|sketch| {
                    if let Some(Shape::Spline { closed, .. } | Shape::Bspline { closed, .. }) =
                        sketch.curve_mut(id).map(|c| &mut c.shape)
                    {
                        *closed = !*closed;
                    }
                });
            }
        }
    });
    message
}

/// A line's length, with its lock, its direction and its two ends.
fn line_rows(
    app: &mut App,
    ui: &mut egui::Ui,
    tool: &mut SketchTool,
    id: CurveId,
    (a, b): (PointId, PointId),
) -> Option<Status> {
    let (p, q) = (tool.sketch.at(a), tool.sketch.at(b));
    let length = gap(p, q);
    let direction = angle_of(p, q).to_degrees().rem_euclid(360.0);
    let dimension = tool.dimension_of(id);
    // With both ends fixed the line cannot change; with only its end fixed a change moves its start.
    let held = tool.all_fixed(&[a, b]);
    let end_held = tool.all_fixed(&[b]);
    let moved_to = |length: f64, direction: f64| {
        let (cos, sin) = (direction.to_radians().cos(), direction.to_radians().sin());
        match end_held {
            true => (a, [q[0] - length * cos, q[1] - length * sin]),
            false => (b, [p[0] + length * cos, p[1] + length * sin]),
        }
    };
    let mut message = None;
    let field = format!("length-{id}");
    if let Some(value) = number(app, ui, (&field, "Length"), LENGTH, length, held) {
        message = match dimension {
            Some(dimension) => tool.retype(&field, dimension, value),
            None => tool.move_points(&field, &[moved_to(value, direction)]),
        };
    }
    if lock(ui, dimension.is_some()) {
        message = tool.toggle(dimension, ConstraintKind::Distance { a, b, value: length });
    }
    ui.end_row();
    let field = format!("angle-{id}");
    if let Some(value) = number(app, ui, (&field, "Angle"), DIRECTION, direction, held) {
        message = tool.move_points(&field, &[moved_to(length, value)]);
    }
    ui.end_row();
    for (point, name) in [(a, "Start"), (b, "End")] {
        message = place_rows(app, ui, tool, point, name).or(message);
    }
    message
}

/// A circle's or an arc's radius, with its lock, and its centre; an arc's `ends` move with them.
fn round_rows(
    app: &mut App,
    ui: &mut egui::Ui,
    tool: &mut SketchTool,
    id: CurveId,
    center: PointId,
    radius: f64,
    ends: Option<(PointId, PointId)>,
) -> Option<Status> {
    let dimension = tool.dimension_of(id);
    let c = tool.sketch.at(center);
    // An arc's radius moves its ends, and its centre carries them along, so a fixed end locks both.
    let carried: Vec<PointId> = ends.map_or(Vec::new(), |(start, end)| vec![start, end]);
    let ends_held = carried.iter().any(|&p| tool.all_fixed(&[p]));
    let centre_held = ends_held || tool.all_fixed(&[center]);
    let mut message = None;
    let field = format!("radius-{id}");
    if let Some(value) = number(app, ui, (&field, "Radius"), LENGTH, radius, ends_held) {
        message = match (dimension, ends) {
            (Some(dimension), _) => tool.retype(&field, dimension, value),
            (None, Some((start, end))) => {
                let out = |p: PointId| {
                    let at = tool.sketch.at(p);
                    let away = gap(c, at).max(1e-12);
                    (p, [c[0] + (at[0] - c[0]) / away * value, c[1] + (at[1] - c[1]) / away * value])
                };
                let moves = [out(start), out(end)];
                tool.move_points(&field, &moves)
            }
            (None, None) => resize_circle(tool, &field, id, value),
        };
    }
    if lock(ui, dimension.is_some()) {
        message = tool.toggle(dimension, ConstraintKind::Radius { curve: id, value: radius });
    }
    ui.end_row();
    // The centre carries an arc's ends along, so the arc moves rather than bends.
    for (axis, name) in [(0, "Centre X"), (1, "Centre Y")] {
        let field = format!("centre-{axis}-{id}");
        if let Some(value) = number(app, ui, (&field, name), POINT, c[axis], centre_held) {
            let delta = value - c[axis];
            let moves: Vec<(PointId, [f64; 2])> = std::iter::once(center)
                .chain(carried.iter().copied())
                .map(|p| {
                    let mut at = tool.sketch.at(p);
                    at[axis] += delta;
                    (p, at)
                })
                .collect();
            message = tool.move_points(&field, &moves);
        }
        ui.end_row();
    }
    message
}

/// A free circle's radius or an ellipse's minor half-axis set straight, since no point lies on it to
/// move.
pub(super) fn resize_circle(tool: &mut SketchTool, field: &str, id: CurveId, value: f64) -> Option<Status> {
    let before = tool.sketch.clone();
    if let Some(size) = tool.sketch.curve_mut(id).and_then(|c| c.shape.own_size_mut()) {
        *size = value;
    }
    if !tool.sketch.solve() {
        tool.sketch = before;
        return Some(Status::Warning("The constraints hold its size".into()));
    }
    tool.remember_field(before, field);
    tool.refresh();
    None
}

/// A point's two coordinates as rows named after `name`.
pub(super) fn place_rows(
    app: &mut App,
    ui: &mut egui::Ui,
    tool: &mut SketchTool,
    point: PointId,
    name: &str,
) -> Option<Status> {
    let mut message = None;
    let at = tool.sketch.at(point);
    let held = tool.all_fixed(&[point]);
    for (axis, letter) in [(0, "X"), (1, "Y")] {
        let field = format!("point-{axis}-{point}");
        let label = if name.is_empty() { letter.to_string() } else { format!("{name} {letter}") };
        if let Some(value) = number(app, ui, (&field, &label), POINT, at[axis], held) {
            let mut to = at;
            to[axis] = value;
            message = tool.move_points(&field, &[(point, to)]);
        }
        ui.end_row();
    }
    message
}

pub(crate) fn point(app: &mut App, ui: &mut egui::Ui, tool: &mut SketchTool, id: PointId) -> Option<Status> {
    let fixed = tool.sketch.point(id)?.fixed;
    if fixed {
        let hint = "Copied from the model, so it stays where the model has it.";
        ui.add(egui::Label::new(theme::hint(hint)).selectable(false));
        return None;
    }
    let mut message = None;
    egui::Grid::new(("sketch-inspect-point", id)).num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        message = place_rows(app, ui, tool, id, "");
    });
    if switch(ui, tool.all_fixed(&[id]), "Fixed", "Hold the point where it is") {
        tool.toggle_fix(&[id]);
    }
    message
}
