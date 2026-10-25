//! The constraint buttons: what each puts on the selection, and putting it there.

use super::*;
use simple3d_sketch::{ConstraintKind, Shape};

/// A constraint button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Rule {
    Coincident,
    Horizontal,
    Vertical,
    Parallel,
    Perpendicular,
    Tangent,
    Equal,
    Symmetric,
    Midpoint,
    Fix,
    Dimension,
    HorizontalDimension,
    VerticalDimension,
}

impl Rule {
    pub(crate) const ALL: [Rule; 13] = [
        Rule::Coincident,
        Rule::Horizontal,
        Rule::Vertical,
        Rule::Parallel,
        Rule::Perpendicular,
        Rule::Tangent,
        Rule::Equal,
        Rule::Symmetric,
        Rule::Midpoint,
        Rule::Fix,
        Rule::Dimension,
        Rule::HorizontalDimension,
        Rule::VerticalDimension,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Rule::Coincident => "Coincident",
            Rule::Horizontal => "Horizontal",
            Rule::Vertical => "Vertical",
            Rule::Parallel => "Parallel",
            Rule::Perpendicular => "Perpendicular",
            Rule::Tangent => "Tangent",
            Rule::Equal => "Equal",
            Rule::Symmetric => "Symmetric",
            Rule::Midpoint => "Midpoint",
            Rule::Fix => "Fix",
            Rule::Dimension => "Dimension",
            Rule::HorizontalDimension => "H dimension",
            Rule::VerticalDimension => "V dimension",
        }
    }

    /// What to select for it, for the tooltip of a dimmed button.
    pub(crate) fn needs(self) -> &'static str {
        match self {
            Rule::Coincident => "Two points, or a point and a line, arc or circle it should lie on",
            Rule::Horizontal | Rule::Vertical => "One or more lines",
            Rule::Parallel | Rule::Perpendicular => "Two lines",
            Rule::Tangent => "Two curves: a line and a round one, two round ones, or two meeting at an end",
            Rule::Equal => "Two or more lines, or two or more arcs and circles",
            Rule::Symmetric => "Two points and the line they mirror across",
            Rule::Midpoint => "A point and a line",
            Rule::Fix => "Points or curves to hold where they are",
            Rule::Dimension => "A line, an arc or circle, two points, a point and a line, or two lines",
            Rule::HorizontalDimension | Rule::VerticalDimension => "Two points or a line",
        }
    }
}

impl SketchTool {
    fn selected_points(&self) -> Vec<PointId> {
        self.selection.iter().filter_map(|p| if let Pick::Point(id) = p { Some(*id) } else { None }).collect()
    }

    fn selected_curves(&self) -> Vec<CurveId> {
        self.selection.iter().filter_map(|p| if let Pick::Curve(id) = p { Some(*id) } else { None }).collect()
    }

    fn shape(&self, id: CurveId) -> Option<&Shape> {
        self.sketch.curve(id).map(|c| &c.shape)
    }

    /// A line's ends.
    fn ends(&self, id: CurveId) -> Option<(PointId, PointId)> {
        match self.shape(id)? {
            Shape::Line { a, b } => Some((*a, *b)),
            _ => None,
        }
    }

    /// What `rule` would put on the selection as it is, or `None` when it does not fit.
    pub(crate) fn kinds_for(&self, rule: Rule) -> Option<Vec<ConstraintKind>> {
        use ConstraintKind as K;
        let (points, curves) = (self.selected_points(), self.selected_curves());
        let lines: Vec<CurveId> = curves.iter().copied().filter(|&c| self.ends(c).is_some()).collect();
        let rounds: Vec<CurveId> =
            curves.iter().copied().filter(|&c| self.shape(c).is_some_and(Shape::is_round)).collect();
        let only_lines = !lines.is_empty() && lines.len() == curves.len() && points.is_empty();
        let at = |p: PointId| self.sketch.at(p);
        let kinds = match (rule, points.len(), curves.len()) {
            (Rule::Coincident, 2, 0) => vec![K::Coincident { a: points[0], b: points[1] }],
            (Rule::Coincident, 1, 1) if lines.len() == 1 || rounds.len() == 1 => {
                vec![K::OnCurve { point: points[0], curve: curves[0] }]
            }
            (Rule::Horizontal, _, _) if only_lines => lines.iter().map(|&line| K::Horizontal { line }).collect(),
            (Rule::Vertical, _, _) if only_lines => lines.iter().map(|&line| K::Vertical { line }).collect(),
            (Rule::Parallel, 0, 2) if lines.len() == 2 => vec![K::Parallel { a: lines[0], b: lines[1] }],
            (Rule::Perpendicular, 0, 2) if lines.len() == 2 => vec![K::Perpendicular { a: lines[0], b: lines[1] }],
            (Rule::Tangent, 0, 2) => {
                let inside = match (self.round_of(curves[0]), self.round_of(curves[1])) {
                    (Some((c1, r1)), Some((c2, r2))) => (c1[0] - c2[0]).hypot(c1[1] - c2[1]) < r1.max(r2),
                    _ => false,
                };
                vec![K::Tangent { a: curves[0], b: curves[1], inside }]
            }
            (Rule::Equal, 0, n) if n >= 2 && (lines.len() == n || rounds.len() == n) => {
                curves[1..].iter().map(|&b| K::Equal { a: curves[0], b }).collect()
            }
            (Rule::Symmetric, 2, 1) if lines.len() == 1 => {
                vec![K::Symmetric { a: points[0], b: points[1], line: lines[0] }]
            }
            (Rule::Midpoint, 1, 1) if lines.len() == 1 => vec![K::Midpoint { point: points[0], line: lines[0] }],
            (Rule::Fix, _, _) if !self.selection.is_empty() => {
                let mut ids = points.clone();
                ids.extend(curves.iter().flat_map(|&c| self.shape(c).map(Shape::points).unwrap_or_default()));
                ids.sort_unstable();
                ids.dedup();
                ids.into_iter()
                    .filter(|&p| self.sketch.point(p).is_some_and(|p| !p.fixed))
                    .map(|point| K::Fix { point, at: at(point) })
                    .collect()
            }
            (Rule::Dimension, 0, 1) if lines.len() == 1 => {
                let (a, b) = self.ends(lines[0])?;
                vec![K::Distance { a, b, value: gap(at(a), at(b)) }]
            }
            (Rule::Dimension, 0, 1) if rounds.len() == 1 => {
                vec![K::Radius { curve: rounds[0], value: self.round_of(rounds[0])?.1 }]
            }
            (Rule::Dimension, 2, 0) => {
                vec![K::Distance { a: points[0], b: points[1], value: gap(at(points[0]), at(points[1])) }]
            }
            (Rule::Dimension, 1, 1) if lines.len() == 1 => {
                let (a, b) = self.ends(lines[0])?;
                let (p, q, r) = (at(a), at(b), at(points[0]));
                let value =
                    ((q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])).abs() / gap(p, q).max(1e-12);
                vec![K::PointLineDistance { point: points[0], line: lines[0], value }]
            }
            (Rule::Dimension, 0, 2) if lines.len() == 2 => {
                let direction =
                    |line: CurveId| self.ends(line).map(|(a, b)| [at(b)[0] - at(a)[0], at(b)[1] - at(a)[1]]);
                let value =
                    simple3d_sketch::solver::angle_between(direction(lines[0])?, direction(lines[1])?).to_degrees();
                vec![K::Angle { a: lines[0], b: lines[1], value }]
            }
            (Rule::HorizontalDimension | Rule::VerticalDimension, n, m)
                if (n, m) == (2, 0) || (n, m, lines.len()) == (0, 1, 1) =>
            {
                let (a, b) = if n == 2 { (points[0], points[1]) } else { self.ends(lines[0])? };
                let axis = if rule == Rule::HorizontalDimension { 0 } else { 1 };
                let value = (at(b)[axis] - at(a)[axis]).abs();
                vec![match axis {
                    0 => K::HorizontalDistance { a, b, value },
                    _ => K::VerticalDistance { a, b, value },
                }]
            }
            _ => return None,
        };
        (!kinds.is_empty()).then_some(kinds)
    }

    /// A round curve's centre and radius.
    fn round_of(&self, id: CurveId) -> Option<([f64; 2], f64)> {
        match *self.shape(id)? {
            Shape::Circle { center, radius } => Some((self.sketch.at(center), radius)),
            Shape::Arc { center, start, .. } => {
                Some((self.sketch.at(center), gap(self.sketch.at(center), self.sketch.at(start))))
            }
            _ => None,
        }
    }

    /// Put `rule` on the selection as one undo step; what it added, or why it was refused, in which
    /// case nothing is added and the constraints it would fight are marked.
    pub(crate) fn apply(&mut self, rule: Rule) -> Result<usize, String> {
        let kinds =
            self.kinds_for(rule).ok_or_else(|| format!("{}: select {}", rule.label(), rule.needs().to_lowercase()))?;
        let before = self.sketch.clone();
        let mut added = Vec::new();
        for kind in kinds {
            match self.sketch.constrain(kind) {
                Ok(id) => added.push(id),
                Err(refusal) => {
                    self.sketch = before;
                    let conflicts = match &refusal {
                        simple3d_sketch::solver::Refusal::Redundant(others) => others.clone(),
                        _ => Vec::new(),
                    };
                    self.refresh();
                    self.conflicts = conflicts;
                    return Err(refusal.message());
                }
            }
        }
        self.remember(before);
        self.refresh();
        // A new dimension is selected, so its value can be typed straight away.
        if rule == Rule::Dimension || rule == Rule::HorizontalDimension || rule == Rule::VerticalDimension {
            self.selection = added.iter().map(|&id| Pick::Constraint(id)).collect();
        }
        Ok(added.len())
    }
}

fn gap(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
