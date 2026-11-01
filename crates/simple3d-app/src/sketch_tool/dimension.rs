//! The Dimension tool, as CAD programs have it: click what to measure -- a line, a circle or arc, two
//! points, a point and a line, or two lines -- then click where the dimension goes. Where decides
//! what it measures: beside a slanted line or between two points its true length, above or below
//! them how far across, left or right of them how far up. A circle gets its diameter, an arc its
//! radius, two lines their angle (or their gap when parallel). Should the other constraints already
//! decide that size, it goes on as a reference dimension that only measures.

use super::constrain::gap;
use super::*;
use crate::app::Status;
use crate::view::View;
use simple3d_sketch::solver::Refusal;
use simple3d_sketch::{ConstraintKind, Shape};

/// What the picks so far measure, once they are enough.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Measure {
    /// A line's ends, or two points.
    Between(PointId, PointId),
    PointLine(PointId, CurveId),
    Lines(CurveId, CurveId),
    Round(CurveId),
}

impl SketchTool {
    /// What the selection measures, when it is a whole set.
    pub(crate) fn measure(&self) -> Option<Measure> {
        let n = self.counts();
        if n.constraints > 0 || n.elliptic + n.splines > 0 {
            return None;
        }
        let (points, curves) = (self.selected_points(), self.selected_curves());
        match (points.len(), n.lines, n.rounds) {
            (0, 1, 0) => self.ends(curves[0]).map(|(a, b)| Measure::Between(a, b)),
            (2, 0, 0) => Some(Measure::Between(points[0], points[1])),
            (1, 1, 0) => Some(Measure::PointLine(points[0], curves[0])),
            (0, 2, 0) => Some(Measure::Lines(curves[0], curves[1])),
            (0, 0, 1) => Some(Measure::Round(curves[0])),
            _ => None,
        }
    }

    /// Whether the selection is a whole set or the start of one: a lone point waits for its partner.
    pub(crate) fn dimension_could_fit(&self) -> bool {
        let n = self.counts();
        self.measure().is_some() || (n.points == 1 && n.curves() + n.constraints == 0)
    }

    /// The dimension measuring `measure` placed at `at`, and the place it measures from, which its
    /// label is kept relative to.
    pub(crate) fn dimension_kind(&self, measure: Measure, at: [f64; 2]) -> Option<(ConstraintKind, [f64; 2])> {
        use ConstraintKind as K;
        let sketch = &self.sketch;
        let kind = match measure {
            Measure::Between(a, b) => {
                let (p, q) = (sketch.at(a), sketch.at(b));
                let between = |axis: usize| at[axis] > p[axis].min(q[axis]) && at[axis] < p[axis].max(q[axis]);
                let square = (p[0] - q[0]).abs() < 1e-9 || (p[1] - q[1]).abs() < 1e-9;
                match (between(0), between(1)) {
                    (true, false) if !square => K::HorizontalDistance { a, b, value: 0.0 },
                    (false, true) if !square => K::VerticalDistance { a, b, value: 0.0 },
                    _ => K::Distance { a, b, value: 0.0 },
                }
            }
            Measure::PointLine(point, line) => K::PointLineDistance { point, line, value: 0.0 },
            Measure::Lines(a, b) => {
                let (p, q) = self.ends(a).map(|(p, q)| (sketch.at(p), sketch.at(q)))?;
                let (r, t) = self.ends(b).map(|(r, t)| (sketch.at(r), sketch.at(t)))?;
                let (d, e) = ([q[0] - p[0], q[1] - p[1]], [t[0] - r[0], t[1] - r[1]]);
                let cross = d[0] * e[1] - d[1] * e[0];
                if cross.abs() < 1e-9 * gap(p, q) * gap(r, t) {
                    let (point, _) = self.ends(b)?;
                    K::PointLineDistance { point, line: a, value: 0.0 }
                } else {
                    K::Angle { a, b, value: 0.0 }
                }
            }
            Measure::Round(curve) => match self.shape(curve)? {
                Shape::Circle { .. } => K::Diameter { curve, value: 0.0 },
                _ => K::Radius { curve, value: 0.0 },
            },
        };
        let mut kind = kind;
        let value = kind.measure(sketch)?;
        *kind.value_mut()? = value;
        let anchor = anchor(sketch, &kind)?;
        Some((kind, anchor))
    }
}

/// The place a dimension measures from, which its label is kept relative to: the middle of what it
/// measures, a round curve's centre, or where two lines meet.
pub(crate) fn anchor(sketch: &Sketch, kind: &ConstraintKind) -> Option<[f64; 2]> {
    use ConstraintKind::*;
    let mid = |p: [f64; 2], q: [f64; 2]| [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0];
    let line = |id: CurveId| match sketch.curve(id)?.shape {
        Shape::Line { a, b } => Some((sketch.at(a), sketch.at(b))),
        _ => None,
    };
    Some(match *kind {
        Distance { a, b, .. } | HorizontalDistance { a, b, .. } | VerticalDistance { a, b, .. } => {
            mid(sketch.at(a), sketch.at(b))
        }
        PointLineDistance { point, line: l, .. } => {
            let r = sketch.at(point);
            mid(r, foot(line(l)?, r))
        }
        Radius { curve, .. } | Diameter { curve, .. } => match sketch.curve(curve)?.shape {
            Shape::Circle { center, .. } | Shape::Arc { center, .. } => sketch.at(center),
            _ => return None,
        },
        Angle { a, b, .. } => {
            let (p, q) = line(a)?;
            let (r, t) = line(b)?;
            meet((p, q), (r, t)).unwrap_or(mid(mid(p, q), mid(r, t)))
        }
        _ => return None,
    })
}

/// Where `r` drops onto the line through `p` and `q`.
pub(crate) fn foot((p, q): ([f64; 2], [f64; 2]), r: [f64; 2]) -> [f64; 2] {
    let d = [q[0] - p[0], q[1] - p[1]];
    let t = ((r[0] - p[0]) * d[0] + (r[1] - p[1]) * d[1]) / (d[0] * d[0] + d[1] * d[1]).max(1e-24);
    [p[0] + d[0] * t, p[1] + d[1] * t]
}

/// Where the lines through `p`-`q` and `r`-`t` cross; `None` when they are parallel.
pub(crate) fn meet((p, q): ([f64; 2], [f64; 2]), (r, t): ([f64; 2], [f64; 2])) -> Option<[f64; 2]> {
    let (d, e) = ([q[0] - p[0], q[1] - p[1]], [t[0] - r[0], t[1] - r[1]]);
    let cross = d[0] * e[1] - d[1] * e[0];
    if cross.abs() < 1e-12 {
        return None;
    }
    let s = ((r[0] - p[0]) * e[1] - (r[1] - p[1]) * e[0]) / cross;
    Some([p[0] + d[0] * s, p[1] + d[1] * s])
}

/// The Dimension tool's click: a point or curve joins the picks, a second click on one drops it, and
/// once the picks are enough a click on empty space -- or anywhere, with two picked -- places the
/// dimension there. What to say, and the new dimension, whose value is then typed.
pub(crate) fn dimension_at(tool: &mut SketchTool, view: &View, cursor: egui::Pos2) -> (Status, Option<ConstraintId>) {
    let pick = tool.pick_at(view, cursor).filter(|p| !matches!(p, Pick::Constraint(_)));
    let whole = tool.measure();
    if let (Some(measure), Some(at)) = (whole, tool.under(view, cursor)) {
        if pick.is_none() || tool.selection.len() >= 2 {
            return place(tool, measure, at);
        }
    }
    let Some(pick) = pick else {
        return (Status::Info(Draw::Dimension.hint().into()), None);
    };
    if let Some(at) = tool.selection.iter().position(|p| *p == pick) {
        tool.selection.remove(at);
    } else {
        tool.selection.push(pick);
        if !tool.dimension_could_fit() {
            tool.selection = vec![pick];
            if !tool.dimension_could_fit() {
                tool.selection.clear();
                return (Status::Warning("Click a line, a circle or arc, or a point".into()), None);
            }
        }
    }
    let next = match tool.measure() {
        Some(Measure::Between(..) | Measure::Round(_)) if tool.selection.len() == 1 => {
            "Click where the dimension goes, or another line or a point to measure against"
        }
        Some(_) => "Click where the dimension goes",
        None if tool.selection.is_empty() => Draw::Dimension.hint(),
        None => "Now click a second point, or a line",
    };
    (Status::Info(next.into()), None)
}

/// Put the dimension measuring `measure` down at `at`: driving when it can be, else as a reference
/// dimension. One that is already there is picked again for its value to be typed.
fn place(tool: &mut SketchTool, measure: Measure, at: [f64; 2]) -> (Status, Option<ConstraintId>) {
    let Some((kind, from)) = tool.dimension_kind(measure, at) else {
        return (Status::Warning("That cannot be dimensioned".into()), None);
    };
    let label = Some([at[0] - from[0], at[1] - from[1]]);
    let same = |k: &ConstraintKind| std::mem::discriminant(k) == std::mem::discriminant(&kind) && same_names(k, &kind);
    if let Some(existing) = tool.sketch.constraints.iter().find(|c| same(&c.kind)).map(|c| c.id) {
        tool.selection = vec![Pick::Constraint(existing)];
        return (Status::Info("Already dimensioned; type its value".into()), Some(existing));
    }
    let before = tool.sketch.clone();
    let (id, said) = match tool.sketch.constrain(kind.clone()) {
        Ok(id) => (id, "Dimensioned; type its value".to_string()),
        Err(Refusal::Redundant(_)) => match tool.sketch.add_reference(kind) {
            Ok(id) => (id, "The other constraints already decide that size, so it only measures it".into()),
            Err(why) => return (Status::Warning(why.message()), None),
        },
        Err(why) => return (Status::Warning(why.message()), None),
    };
    if let Some(constraint) = tool.sketch.constraint_mut(id) {
        constraint.label = label;
    }
    tool.remember(before);
    tool.refresh();
    tool.selection = vec![Pick::Constraint(id)];
    (Status::Info(said), Some(id))
}

/// Whether two dimensions of one kind name the same things, either way round.
fn same_names(a: &ConstraintKind, b: &ConstraintKind) -> bool {
    let sorted = |mut v: Vec<u32>| {
        v.sort_unstable();
        v
    };
    sorted(a.points()) == sorted(b.points()) && sorted(a.curves()) == sorted(b.curves())
}
