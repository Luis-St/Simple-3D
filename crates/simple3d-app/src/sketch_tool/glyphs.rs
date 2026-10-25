//! Constraint marks and dimension labels: where each goes and what it says. Every constraint gets a
//! label, so each can be clicked to select it and deleted, not only those with a value.

use super::*;
use crate::view::View;
use simple3d_core::unit::{format_angle, format_length, Unit};
use simple3d_sketch::{ConstraintKind, Shape};
use std::collections::HashMap;

/// A label's text height in pixels, and a character's width, for hit testing without fonts.
const TEXT_HEIGHT: f32 = 14.0;
const CHAR_WIDTH: f32 = 7.0;

/// One constraint's label on screen.
pub(crate) struct Label {
    pub id: ConstraintId,
    pub text: String,
    /// Its top left corner on screen.
    pub at: egui::Pos2,
    /// Whether it is a dimension, drawn in the measuring colour.
    pub dimension: bool,
}

impl Label {
    pub(crate) fn rect(&self) -> egui::Rect {
        egui::Rect::from_min_size(self.at, egui::vec2(CHAR_WIDTH * self.text.chars().count() as f32, TEXT_HEIGHT))
            .expand(3.0)
    }
}

/// The middle of a curve: a line's midpoint, a round curve's rim at 45 degrees, a spline's first point.
fn middle(sketch: &Sketch, curve: CurveId) -> [f64; 2] {
    let Some(curve) = sketch.curve(curve) else { return [0.0; 2] };
    match curve.shape {
        Shape::Line { a, b } => {
            let (a, b) = (sketch.at(a), sketch.at(b));
            [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]
        }
        Shape::Circle { center, radius } => {
            let c = sketch.at(center);
            [c[0] + radius * std::f64::consts::FRAC_1_SQRT_2, c[1] + radius * std::f64::consts::FRAC_1_SQRT_2]
        }
        Shape::Arc { center, start, .. } => {
            let (c, s) = (sketch.at(center), sketch.at(start));
            let r = (s[0] - c[0]).hypot(s[1] - c[1]);
            [c[0] + r * std::f64::consts::FRAC_1_SQRT_2, c[1] + r * std::f64::consts::FRAC_1_SQRT_2]
        }
        Shape::Spline { ref points, .. } | Shape::Bspline { ref points, .. } => sketch.at(points[0]),
    }
}

/// What a constraint's label says and the sketch point it hangs from.
fn describe(sketch: &Sketch, kind: &ConstraintKind, unit: Unit) -> (String, [f64; 2]) {
    use ConstraintKind::*;
    let mid = |a: PointId, b: PointId| {
        let (p, q) = (sketch.at(a), sketch.at(b));
        [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0]
    };
    let length = |v: f64| format_length(v, unit);
    match *kind {
        Coincident { a, .. } => ("C".into(), sketch.at(a)),
        Horizontal { line } => ("H".into(), middle(sketch, line)),
        Vertical { line } => ("V".into(), middle(sketch, line)),
        Parallel { a, .. } => ("||".into(), middle(sketch, a)),
        Perpendicular { a, .. } => ("L".into(), middle(sketch, a)),
        Tangent { a, .. } => ("T".into(), middle(sketch, a)),
        Equal { a, .. } => ("=".into(), middle(sketch, a)),
        Symmetric { a, b, .. } => ("S".into(), mid(a, b)),
        Midpoint { point, .. } => ("M".into(), sketch.at(point)),
        OnCurve { point, .. } => ("O".into(), sketch.at(point)),
        Fix { point, .. } => ("F".into(), sketch.at(point)),
        Distance { a, b, value } => (length(value), mid(a, b)),
        HorizontalDistance { a, b, value } => (format!("H {}", length(value)), mid(a, b)),
        VerticalDistance { a, b, value } => (format!("V {}", length(value)), mid(a, b)),
        PointLineDistance { point, line, value } => {
            let (p, m) = (sketch.at(point), middle(sketch, line));
            (length(value), [(p[0] + m[0]) / 2.0, (p[1] + m[1]) / 2.0])
        }
        Radius { curve, value } => (format!("R {}", length(value)), middle(sketch, curve)),
        Angle { a, b, value } => {
            let (p, q) = (middle(sketch, a), middle(sketch, b));
            (format!("{}\u{00B0}", format_angle(value.abs())), [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0])
        }
    }
}

/// Every constraint's label, those hanging from one place stacked so they do not cover each other.
pub(crate) fn labels(tool: &SketchTool, view: &View) -> Vec<Label> {
    let mut stacked: HashMap<(i32, i32), f32> = HashMap::new();
    let mut out = Vec::new();
    for constraint in &tool.sketch.constraints {
        let (text, anchor) = describe(&tool.sketch, &constraint.kind, tool.unit);
        let Some(screen) = tool.screen(view, anchor) else { continue };
        let key = ((screen.x / 12.0) as i32, (screen.y / 12.0) as i32);
        let shift = stacked.entry(key).or_insert(0.0);
        let at = screen + egui::vec2(8.0, -TEXT_HEIGHT - 6.0 - *shift);
        *shift += TEXT_HEIGHT + 4.0;
        out.push(Label { id: constraint.id, text, at, dimension: constraint.kind.value().is_some() });
    }
    out
}

/// The constraint whose label is under the pointer.
pub(crate) fn label_at(tool: &SketchTool, view: &View, cursor: egui::Pos2) -> Option<ConstraintId> {
    labels(tool, view).into_iter().find(|label| label.rect().contains(cursor)).map(|label| label.id)
}
