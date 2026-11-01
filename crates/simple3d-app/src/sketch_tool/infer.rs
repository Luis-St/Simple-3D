//! Constraints inferred while drawing, as CAD programs put them on: a click that lands on a curve
//! stays on it, one on a line's middle stays there, and a line drawn nearly level, upright, square to
//! the line it starts from or running on from an arc snaps to that and is held so. The pointer
//! shows what a click would infer before it is made; holding Ctrl draws freely, inferring nothing.

use super::constrain::gap;
use super::hit::{CURVE_REACH, POINT_REACH};
use super::*;
use crate::icon::Glyph;
use crate::view::View;
use simple3d_sketch::{sample_curve, ConstraintKind, Shape};

/// How far off a direction a line may be drawn and still snap to it, in degrees.
const SNAP_ANGLE: f64 = 4.0;
/// Two places this close, in millimetres, are one.
const SAME: f64 = 1e-6;

/// A constraint a drawing click would put on what it adds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Infer {
    /// The new point at `at` on a curve.
    OnCurve {
        at: [f64; 2],
        curve: CurveId,
    },
    /// The new point at `at` in the middle of a line.
    Midpoint {
        at: [f64; 2],
        line: CurveId,
    },
    /// The new line from `from` to `to` level.
    Horizontal {
        from: [f64; 2],
        to: [f64; 2],
    },
    Vertical {
        from: [f64; 2],
        to: [f64; 2],
    },
    /// The new line from `from` to `to` square to `line`, which it starts from.
    Perpendicular {
        from: [f64; 2],
        to: [f64; 2],
        line: CurveId,
    },
    /// The new line from `from` to `to` running on smoothly from `curve`, which it starts from.
    Tangent {
        from: [f64; 2],
        to: [f64; 2],
        curve: CurveId,
    },
}

impl Infer {
    pub(crate) fn glyph(self) -> Glyph {
        match self {
            Infer::OnCurve { .. } => Glyph::ConstrainOnCurve,
            Infer::Midpoint { .. } => Glyph::ConstrainMidpoint,
            Infer::Horizontal { .. } => Glyph::ConstrainHorizontal,
            Infer::Vertical { .. } => Glyph::ConstrainVertical,
            Infer::Perpendicular { .. } => Glyph::ConstrainPerpendicular,
            Infer::Tangent { .. } => Glyph::ConstrainTangent,
        }
    }

    /// The same, for a line ending at `to` instead.
    fn ending(self, to: [f64; 2]) -> Infer {
        match self {
            Infer::Horizontal { from, .. } => Infer::Horizontal { from, to },
            Infer::Vertical { from, .. } => Infer::Vertical { from, to },
            Infer::Perpendicular { from, line, .. } => Infer::Perpendicular { from, to, line },
            Infer::Tangent { from, curve, .. } => Infer::Tangent { from, to, curve },
            other => other,
        }
    }

    /// Where the point it holds is, for those on a point.
    fn waits_at(self) -> Option<[f64; 2]> {
        match self {
            Infer::OnCurve { at, .. } | Infer::Midpoint { at, .. } => Some(at),
            _ => None,
        }
    }
}

fn near(a: [f64; 2], b: [f64; 2]) -> bool {
    gap(a, b) < SAME
}

impl SketchTool {
    /// Where a drawing click lands and what it infers: [`SketchTool::click_at`], then, unless drawing
    /// freely, on the grid or on a point, snapped onto a direction and a curve.
    pub(crate) fn drawing_click(
        &self,
        view: &View,
        cursor: egui::Pos2,
        spacing: Option<f64>,
    ) -> Option<(Click, Vec<Infer>)> {
        let click = self.click_at(view, cursor, spacing)?;
        if self.free || spacing.is_some() || click.point.is_some() || !self.draw.draws() {
            return Some((click, Vec::new()));
        }
        let mut at = click.at;
        let mut out = Vec::new();
        let ray = self.direction(at);
        if let Some((from, d, infer)) = ray {
            at = along(from, d, at);
            out.push(infer.ending(at));
        }
        if let Some(curve) = self.curve_at(view, cursor) {
            let shape = self.shape(curve).cloned();
            let mid = match shape {
                Some(Shape::Line { a, b }) => Some(mid(self.sketch.at(a), self.sketch.at(b))),
                _ => None,
            };
            let on_mid = mid.and_then(|m| self.screen(view, m)).is_some_and(|m| m.distance(cursor) <= POINT_REACH);
            // A line's direction and a curve crossing it: both, where they meet.
            let crossing = match (ray, shape) {
                (Some((from, d, _)), Some(Shape::Line { a, b })) => super::dimension::meet(
                    (from, [from[0] + d[0], from[1] + d[1]]),
                    (self.sketch.at(a), self.sketch.at(b)),
                )
                .filter(|&x| self.screen(view, x).is_some_and(|x| x.distance(cursor) <= CURVE_REACH * 1.5)),
                _ => None,
            };
            if let (true, Some(m)) = (on_mid, mid) {
                at = m;
                out = vec![Infer::Midpoint { at, line: curve }];
            } else if let Some(x) = crossing {
                at = x;
                if let Some((_, _, infer)) = ray {
                    out = vec![infer.ending(at)];
                }
                out.push(Infer::OnCurve { at, curve });
            } else if let Some(onto) = self.onto(curve, self.under(view, cursor)?) {
                at = onto;
                out = vec![Infer::OnCurve { at, curve }];
            }
        }
        Some((Click { at, point: None }, out))
    }

    /// The direction a line drawn from the last click to `at` snaps to, if it is near one: level,
    /// upright, square to a line it starts from or on from an arc it starts from. Its start, the
    /// direction, and what it infers, still to be given where the line ends.
    fn direction(&self, at: [f64; 2]) -> Option<([f64; 2], [f64; 2], Infer)> {
        if self.draw != Draw::Line {
            return None;
        }
        let start = *self.clicks.last()?;
        let from = start.at;
        let v = [at[0] - from[0], at[1] - from[1]];
        let length = v[0].hypot(v[1]);
        if length < SAME {
            return None;
        }
        let to = at;
        let mut candidates: Vec<([f64; 2], Infer)> =
            vec![([1.0, 0.0], Infer::Horizontal { from, to }), ([0.0, 1.0], Infer::Vertical { from, to })];
        if let Some(p) = start.point {
            for curve in &self.sketch.curves {
                let id = curve.id;
                match curve.shape {
                    Shape::Line { a, b } if a == p || b == p => {
                        let (q, r) = (self.sketch.at(a), self.sketch.at(b));
                        let d = [r[0] - q[0], r[1] - q[1]];
                        let n = d[0].hypot(d[1]).max(1e-12);
                        let normal = [-d[1] / n, d[0] / n];
                        candidates.push((normal, Infer::Perpendicular { from, to, line: id }));
                    }
                    Shape::Arc { center, start, end } if start == p || end == p => {
                        let c = self.sketch.at(center);
                        let r = [from[0] - c[0], from[1] - c[1]];
                        let n = r[0].hypot(r[1]).max(1e-12);
                        let tangent = [-r[1] / n, r[0] / n];
                        candidates.push((tangent, Infer::Tangent { from, to, curve: id }));
                    }
                    _ => {}
                }
            }
        }
        let off = |d: [f64; 2]| ((v[0] * d[0] + v[1] * d[1]).abs() / length).clamp(0.0, 1.0).acos();
        let (d, infer) = candidates
            .into_iter()
            .map(|(d, infer)| (off(d), d, infer))
            .filter(|(angle, _, _)| *angle < SNAP_ANGLE.to_radians())
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, d, infer)| (d, infer))?;
        Some((from, d, infer))
    }

    /// The place on curve `id` nearest `at`, for curves a point can be held on.
    fn onto(&self, id: CurveId, at: [f64; 2]) -> Option<[f64; 2]> {
        let curve = self.sketch.curve(id)?;
        match curve.shape {
            Shape::Line { a, b } => Some(super::dimension::foot((self.sketch.at(a), self.sketch.at(b)), at)),
            Shape::Circle { .. } | Shape::Arc { .. } => {
                let (c, r) = self.round_of(id)?;
                let d = [at[0] - c[0], at[1] - c[1]];
                let n = d[0].hypot(d[1]);
                (n > SAME).then(|| [c[0] + d[0] / n * r, c[1] + d[1] / n * r])
            }
            Shape::Ellipse { .. } | Shape::EllipseArc { .. } => {
                let samples = sample_curve(&self.sketch, curve, 256).points;
                samples.into_iter().min_by(|p, q| gap(*p, at).total_cmp(&gap(*q, at)))
            }
            Shape::Spline { .. } | Shape::Bspline { .. } => None,
        }
    }

    /// Take a drawing click with what it infers: the click as ever, then each inferred constraint
    /// put on what the click added, as part of the same undo step. Those on a point not yet added
    /// wait for the click that adds it.
    pub(crate) fn click_inferred(&mut self, click: Click, inferred: Vec<Infer>) {
        let newest_point = self.sketch.points.iter().map(|p| p.id).max().unwrap_or(0);
        let newest_curve = self.sketch.curves.iter().map(|c| c.id).max().unwrap_or(0);
        self.inferred.extend(inferred);
        self.click(click);
        let inferred = std::mem::take(&mut self.inferred);
        let mut added = false;
        for infer in &inferred {
            if let Some(kind) = self.inferred_kind(*infer, newest_point, newest_curve) {
                added |= self.sketch.constrain(kind).is_ok();
            }
        }
        if added {
            self.refresh();
        }
        let waiting = |at: [f64; 2]| self.clicks.iter().any(|c| c.point.is_none() && near(c.at, at));
        self.inferred = inferred.into_iter().filter(|i| i.waits_at().is_some_and(waiting)).collect();
    }

    /// The constraint `infer` stands for on what is newer than the ids given, if the click added it.
    fn inferred_kind(&self, infer: Infer, newest_point: PointId, newest_curve: CurveId) -> Option<ConstraintKind> {
        let new_point =
            |at: [f64; 2]| self.sketch.points.iter().find(|p| p.id > newest_point && near(p.at, at)).map(|p| p.id);
        let new_line = |from: [f64; 2], to: [f64; 2]| {
            self.sketch.curves.iter().filter(|c| c.id > newest_curve).find_map(|c| match c.shape {
                Shape::Line { a, b } => {
                    let (p, q) = (self.sketch.at(a), self.sketch.at(b));
                    ((near(p, from) && near(q, to)) || (near(p, to) && near(q, from))).then_some(c.id)
                }
                _ => None,
            })
        };
        let owns = |curve: CurveId, point: PointId| self.shape(curve).is_some_and(|s| s.points().contains(&point));
        Some(match infer {
            Infer::OnCurve { at, curve } => {
                let point = new_point(at).filter(|&p| !owns(curve, p))?;
                ConstraintKind::OnCurve { point, curve }
            }
            Infer::Midpoint { at, line } => {
                let point = new_point(at).filter(|&p| !owns(line, p))?;
                ConstraintKind::Midpoint { point, line }
            }
            Infer::Horizontal { from, to } => ConstraintKind::Horizontal { line: new_line(from, to)? },
            Infer::Vertical { from, to } => ConstraintKind::Vertical { line: new_line(from, to)? },
            Infer::Perpendicular { from, to, line } => {
                ConstraintKind::Perpendicular { a: line, b: new_line(from, to)? }
            }
            Infer::Tangent { from, to, curve } => {
                ConstraintKind::Tangent { a: curve, b: new_line(from, to)?, inside: false }
            }
        })
    }
}

/// `at` moved onto the line from `from` along unit direction `d`.
fn along(from: [f64; 2], d: [f64; 2], at: [f64; 2]) -> [f64; 2] {
    let t = (at[0] - from[0]) * d[0] + (at[1] - from[1]) * d[1];
    [from[0] + d[0] * t, from[1] + d[1] * t]
}

fn mid(p: [f64; 2], q: [f64; 2]) -> [f64; 2] {
    [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0]
}
