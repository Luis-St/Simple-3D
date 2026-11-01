//! Constraint marks: each geometric constraint as a small icon beside what it holds -- on both curves
//! of a pair, as CAD programs mark them -- and a coincidence as a dot on its point. Every mark and
//! every dimension's label can be clicked to select it and deleted; pointing at one lights up what
//! it holds.

use super::dimension_draw::{self, DimShape};
use super::*;
use crate::icon::{self, Glyph};
use crate::theme::token;
use crate::view::View;
use simple3d_sketch::{ConstraintKind, Shape};
use std::collections::{HashMap, HashSet};

/// A mark's side, in pixels, and how far it hangs from what it marks.
const MARK: f32 = 16.0;
const HANG: egui::Vec2 = egui::vec2(12.0, -12.0);

/// One mark on screen.
pub(crate) struct Mark {
    pub id: ConstraintId,
    pub glyph: Glyph,
    pub rect: egui::Rect,
}

/// The middle of a curve: a line's midpoint, a round curve's or ellipse's rim at 45 degrees, a spline's
/// first point.
fn middle(sketch: &Sketch, curve: CurveId) -> [f64; 2] {
    let Some(curve) = sketch.curve(curve) else { return [0.0; 2] };
    let rim =
        |c: [f64; 2], r: f64| [c[0] + r * std::f64::consts::FRAC_1_SQRT_2, c[1] + r * std::f64::consts::FRAC_1_SQRT_2];
    match curve.shape {
        Shape::Line { a, b } => {
            let (a, b) = (sketch.at(a), sketch.at(b));
            [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]
        }
        Shape::Circle { center, radius } => rim(sketch.at(center), radius),
        Shape::Arc { center, start, .. } => {
            let (c, s) = (sketch.at(center), sketch.at(start));
            rim(c, (s[0] - c[0]).hypot(s[1] - c[1]))
        }
        Shape::Ellipse { center, major, minor } | Shape::EllipseArc { center, major, minor, .. } => {
            simple3d_sketch::Ellipse::new(sketch.at(center), sketch.at(major), minor).at(std::f64::consts::FRAC_PI_4)
        }
        Shape::Spline { ref points, .. } | Shape::Bspline { ref points, .. } => sketch.at(points[0]),
    }
}

/// A geometric constraint's glyph and the places it is marked at; nothing for a dimension, whose
/// label is its mark, or a coincidence, drawn as a dot.
fn marked(sketch: &Sketch, kind: &ConstraintKind) -> Option<(Glyph, Vec<[f64; 2]>)> {
    use ConstraintKind::*;
    let on = |c: CurveId| middle(sketch, c);
    Some(match *kind {
        Horizontal { line } => (Glyph::ConstrainHorizontal, vec![on(line)]),
        Vertical { line } => (Glyph::ConstrainVertical, vec![on(line)]),
        Parallel { a, b } => (Glyph::ConstrainParallel, vec![on(a), on(b)]),
        Perpendicular { a, b } => (Glyph::ConstrainPerpendicular, vec![on(a), on(b)]),
        Tangent { a, b, .. } => (Glyph::ConstrainTangent, vec![on(a), on(b)]),
        Equal { a, b } => (Glyph::ConstrainEqual, vec![on(a), on(b)]),
        Symmetric { a, b, .. } => (Glyph::ConstrainSymmetric, vec![sketch.at(a), sketch.at(b)]),
        Midpoint { point, .. } => (Glyph::ConstrainMidpoint, vec![sketch.at(point)]),
        OnCurve { point, .. } => (Glyph::ConstrainOnCurve, vec![sketch.at(point)]),
        Fix { point, .. } => (Glyph::ConstrainFix, vec![sketch.at(point)]),
        _ => return None,
    })
}

/// Every geometric constraint's marks, those hanging from one place side by side.
pub(crate) fn marks(tool: &SketchTool, view: &View) -> Vec<Mark> {
    let mut beside: HashMap<(i32, i32), f32> = HashMap::new();
    let mut out = Vec::new();
    for constraint in &tool.sketch.constraints {
        let Some((glyph, places)) = marked(&tool.sketch, &constraint.kind) else { continue };
        for place in places {
            let Some(screen) = tool.screen(view, place) else { continue };
            let key = ((screen.x / 12.0) as i32, (screen.y / 12.0) as i32);
            let shift = beside.entry(key).or_insert(0.0);
            let centre = screen + HANG + egui::vec2(*shift, 0.0);
            *shift += MARK + 2.0;
            out.push(Mark {
                id: constraint.id,
                glyph,
                rect: egui::Rect::from_center_size(centre, egui::Vec2::splat(MARK)),
            });
        }
    }
    out
}

/// The constraint whose mark or dimension label is under the pointer.
pub(crate) fn label_at(tool: &SketchTool, view: &View, cursor: egui::Pos2) -> Option<ConstraintId> {
    let dimension = dimension_draw::all(tool, view).into_iter().find(|(_, d)| d.rect().contains(cursor));
    dimension
        .map(|(id, _)| id)
        .or_else(|| marks(tool, view).into_iter().find(|m| m.rect.contains(cursor)).map(|m| m.id))
}

/// The constraints pointed at -- in the sketch or in the list -- or selected.
pub(crate) fn pointed(tool: &SketchTool) -> HashSet<ConstraintId> {
    let mut ids: HashSet<ConstraintId> = tool.list_hover.into_iter().collect();
    for pick in tool.selection.iter().chain(tool.hover.as_ref()) {
        if let Pick::Constraint(id) = *pick {
            ids.insert(id);
        }
    }
    ids
}

/// The curves and points the constraints pointed at hold, lit up as the constraints are.
pub(crate) fn lit(tool: &SketchTool) -> (HashSet<CurveId>, HashSet<PointId>) {
    let (mut curves, mut points) = (HashSet::new(), HashSet::new());
    for id in pointed(tool) {
        if let Some(constraint) = tool.sketch.constraint(id) {
            curves.extend(constraint.kind.curves());
            points.extend(constraint.kind.points());
        }
    }
    (curves, points)
}

/// The colour a constraint's mark or label is drawn in.
fn colour(tool: &SketchTool, id: ConstraintId, dimension: bool, driven: bool) -> egui::Color32 {
    if tool.conflicts.contains(&id) || tool.analysis.redundant.contains(&id) {
        token::DANGER
    } else if pointed(tool).contains(&id) {
        token::ACCENT
    } else if driven {
        token::TEXT_LO
    } else if dimension {
        token::MEASURE
    } else {
        token::TEXT_LO
    }
}

/// Every dimension, mark and coincidence dot.
pub(crate) fn draw(tool: &SketchTool, painter: &egui::Painter, view: &View) {
    for constraint in &tool.sketch.constraints {
        if let ConstraintKind::Coincident { a, .. } = constraint.kind {
            if let Some(at) = tool.screen(view, tool.sketch.at(a)) {
                let ink = colour(tool, constraint.id, false, false);
                painter.circle_stroke(at, 5.0, egui::Stroke::new(1.5_f32, ink));
            }
        }
    }
    for (id, shape) in dimension_draw::all(tool, view) {
        let driven = tool.sketch.constraint(id).is_some_and(|c| c.driven);
        shape.paint(painter, colour(tool, id, true, driven));
    }
    for mark in marks(tool, view) {
        let ink = colour(tool, mark.id, false, false);
        painter.rect_filled(mark.rect, 3.0, token::SURFACE_1.gamma_multiply(0.9));
        painter.rect_stroke(
            mark.rect,
            3.0,
            egui::Stroke::new(1.0_f32, ink.gamma_multiply(0.5)),
            egui::StrokeKind::Inside,
        );
        icon::draw(painter, mark.rect.shrink(2.5), mark.glyph, ink);
    }
}

/// A dimension being placed, drawn as it would go down.
pub(crate) fn draw_preview(painter: &egui::Painter, shape: &DimShape) {
    shape.paint(painter, token::ACCENT);
}
