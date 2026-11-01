//! Dimensions drawn as on a technical drawing: extension lines out from what is measured, a dimension
//! line with arrowheads between them, and the value where the label was put. A radius or diameter
//! runs from the centre through the rim, an angle is an arc between the two lines.

use super::dimension::{anchor, foot, meet};
use super::*;
use crate::theme::token;
use crate::view::View;
use simple3d_core::unit::{format_angle, format_length, Unit};
use simple3d_sketch::{ConstraintKind, Shape};

/// How far from what it measures a dimension sits until it is placed, in pixels.
const ASIDE: f32 = 28.0;
/// An arrowhead's length and half its width, in pixels.
const ARROW: f32 = 8.0;
const ARROW_HALF: f32 = 3.0;
/// A label's text height and a character's width, for laying it out and hit testing without fonts.
const TEXT_HEIGHT: f32 = 14.0;
const CHAR_WIDTH: f32 = 7.0;

/// One dimension as drawn, in screen coordinates.
pub(crate) struct DimShape {
    pub text: String,
    /// The label's centre.
    pub at: egui::Pos2,
    pub lines: Vec<[egui::Pos2; 2]>,
    /// Each arrowhead's tip and the way it points.
    pub arrows: Vec<(egui::Pos2, egui::Vec2)>,
}

impl DimShape {
    pub(crate) fn rect(&self) -> egui::Rect {
        let size = egui::vec2(CHAR_WIDTH * self.text.chars().count() as f32, TEXT_HEIGHT);
        egui::Rect::from_center_size(self.at, size).expand(3.0)
    }

    pub(crate) fn paint(&self, painter: &egui::Painter, colour: egui::Color32) {
        let stroke = egui::Stroke::new(1.0_f32, colour);
        for line in &self.lines {
            painter.line_segment(*line, stroke);
        }
        for &(tip, way) in &self.arrows {
            let way = way.normalized();
            let side = egui::vec2(-way.y, way.x) * ARROW_HALF;
            let back = tip - way * ARROW;
            painter.add(egui::Shape::convex_polygon(vec![tip, back + side, back - side], colour, egui::Stroke::NONE));
        }
        let rect = self.rect();
        painter.rect_filled(rect, 3.0, token::SURFACE_1.gamma_multiply(0.85));
        painter.text(rect.center(), egui::Align2::CENTER_CENTER, &self.text, egui::FontId::monospace(11.0), colour);
    }
}

fn add(p: [f64; 2], q: [f64; 2]) -> [f64; 2] {
    [p[0] + q[0], p[1] + q[1]]
}

fn sub(p: [f64; 2], q: [f64; 2]) -> [f64; 2] {
    [p[0] - q[0], p[1] - q[1]]
}

fn scale(p: [f64; 2], s: f64) -> [f64; 2] {
    [p[0] * s, p[1] * s]
}

fn dot(p: [f64; 2], q: [f64; 2]) -> f64 {
    p[0] * q[0] + p[1] * q[1]
}

fn unit(p: [f64; 2], or: [f64; 2]) -> [f64; 2] {
    let length = p[0].hypot(p[1]);
    if length < 1e-12 {
        or
    } else {
        scale(p, 1.0 / length)
    }
}

/// What a dimension says: a length, a radius or diameter with its sign, or an angle; a reference
/// dimension in brackets, as drawings mark one.
pub(crate) fn text(kind: &ConstraintKind, value: f64, driven: bool, unit: Unit) -> String {
    let said = match kind {
        ConstraintKind::Radius { .. } => format!("R {}", format_length(value, unit)),
        ConstraintKind::Diameter { .. } => format!("\u{00D8} {}", format_length(value, unit)),
        ConstraintKind::Angle { .. } => format!("{}\u{00B0}", format_angle(value.abs())),
        _ => format_length(value.abs(), unit),
    };
    if driven {
        format!("({said})")
    } else {
        said
    }
}

/// Dimension `kind` drawn with its label at `label` from the place it measures from, or where it sits
/// until placed; `None` for a constraint that is no dimension or what is off screen.
pub(crate) fn shape(
    tool: &SketchTool,
    view: &View,
    kind: &ConstraintKind,
    label: Option<[f64; 2]>,
    driven: bool,
) -> Option<DimShape> {
    let sketch = &tool.sketch;
    let value = kind.value()?;
    let from = anchor(sketch, kind)?;
    let aside = f64::from(ASIDE) / view.pixels_per_mm().max(1e-9);
    let screen = |p: [f64; 2]| tool.screen(view, p);
    let mut lines: Vec<[[f64; 2]; 2]> = Vec::new();
    let mut arrows: Vec<([f64; 2], [f64; 2])> = Vec::new();
    // A dimension line from `p` to `q`, arrowheads pointing out to both ends.
    fn span(lines: &mut Vec<[[f64; 2]; 2]>, arrows: &mut Vec<([f64; 2], [f64; 2])>, p: [f64; 2], q: [f64; 2]) {
        lines.push([p, q]);
        arrows.push((p, sub(p, q)));
        arrows.push((q, sub(q, p)));
    }
    let off = match *kind {
        ConstraintKind::Distance { a, b, .. }
        | ConstraintKind::HorizontalDistance { a, b, .. }
        | ConstraintKind::VerticalDistance { a, b, .. } => {
            let (p, q) = (sketch.at(a), sketch.at(b));
            let across = match kind {
                ConstraintKind::HorizontalDistance { .. } => [0.0, 1.0],
                ConstraintKind::VerticalDistance { .. } => [1.0, 0.0],
                _ => {
                    let u = unit(sub(q, p), [1.0, 0.0]);
                    [-u[1], u[0]]
                }
            };
            let off = label.unwrap_or(scale(across, aside));
            // Each end carried out across to the dimension line's level.
            let level = dot(add(from, off), across);
            let (pe, qe) =
                (add(p, scale(across, level - dot(p, across))), add(q, scale(across, level - dot(q, across))));
            lines.push([p, pe]);
            lines.push([q, qe]);
            span(&mut lines, &mut arrows, pe, qe);
            off
        }
        ConstraintKind::PointLineDistance { point, line, .. } => {
            let Shape::Line { a, b } = sketch.curve(line)?.shape else { return None };
            let (p, q, r) = (sketch.at(a), sketch.at(b), sketch.at(point));
            let f = foot((p, q), r);
            let along = unit(sub(q, p), [1.0, 0.0]);
            let off = label.unwrap_or(scale(along, aside));
            let shift = scale(along, dot(off, along));
            lines.push([r, add(r, shift)]);
            lines.push([f, add(f, shift)]);
            span(&mut lines, &mut arrows, add(r, shift), add(f, shift));
            off
        }
        ConstraintKind::Radius { curve, .. } | ConstraintKind::Diameter { curve, .. } => {
            let radius = match sketch.curve(curve)?.shape {
                Shape::Circle { radius, .. } => radius,
                Shape::Arc { start, .. } => super::constrain::gap(from, sketch.at(start)),
                _ => return None,
            };
            let off = label.unwrap_or(scale([0.7071, 0.7071], radius + aside));
            let u = unit(off, [1.0, 0.0]);
            let rim = add(from, scale(u, radius));
            match kind {
                ConstraintKind::Diameter { .. } => span(&mut lines, &mut arrows, add(from, scale(u, -radius)), rim),
                _ => {
                    lines.push([from, rim]);
                    arrows.push((rim, u));
                }
            }
            if dot(off, off).sqrt() > radius {
                lines.push([rim, add(from, off)]);
            }
            off
        }
        ConstraintKind::Angle { a, b, .. } => {
            let line = |id: CurveId| match sketch.curve(id)?.shape {
                Shape::Line { a, b } => Some((sketch.at(a), sketch.at(b))),
                _ => None,
            };
            let ((p, q), (r, t)) = (line(a)?, line(b)?);
            let (da, db) = (unit(sub(q, p), [1.0, 0.0]), unit(sub(t, r), [1.0, 0.0]));
            let bisector = unit(add(da, db), [-da[1], da[0]]);
            let off = label.unwrap_or(scale(bisector, aside * 1.5));
            if meet((p, q), (r, t)).is_some() {
                // Both rays turned round together when the label is behind them, which keeps the
                // angle between them the one the dimension holds.
                let flip = if dot(off, add(da, db)) < 0.0 { -1.0 } else { 1.0 };
                let (ra, rb) = (scale(da, flip), scale(db, flip));
                let reach = dot(off, off).sqrt();
                let turn = simple3d_sketch::solver::angle_between(ra, rb);
                let start = ra[1].atan2(ra[0]);
                let arc: Vec<[f64; 2]> = (0..=24)
                    .map(|k| {
                        let angle = start + turn * k as f64 / 24.0;
                        add(from, [reach * angle.cos(), reach * angle.sin()])
                    })
                    .collect();
                lines.extend(arc.windows(2).map(|w| [w[0], w[1]]));
                lines.push([from, add(from, scale(ra, reach))]);
                lines.push([from, add(from, scale(rb, reach))]);
                let (first, last) = (arc[0], arc[arc.len() - 1]);
                arrows.push((first, sub(first, arc[1])));
                arrows.push((last, sub(last, arc[arc.len() - 2])));
            }
            off
        }
        _ => return None,
    };
    let at = screen(add(from, off))?;
    let mut drawn = DimShape { text: text(kind, value, driven, tool.unit), at, lines: Vec::new(), arrows: Vec::new() };
    for [p, q] in lines {
        drawn.lines.push([screen(p)?, screen(q)?]);
    }
    for (tip, way) in arrows {
        let (s, t) = (screen(tip)?, screen(add(tip, scale(unit(way, [1.0, 0.0]), aside * 0.1)))?);
        drawn.arrows.push((s, t - s));
    }
    Some(drawn)
}

/// Every dimension in the sketch as drawn, by constraint.
pub(crate) fn all(tool: &SketchTool, view: &View) -> Vec<(ConstraintId, DimShape)> {
    tool.sketch
        .constraints
        .iter()
        .filter_map(|c| Some((c.id, shape(tool, view, &c.kind, c.label, c.driven)?)))
        .collect()
}
