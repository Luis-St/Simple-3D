//! The geometric constraints as tools: what each puts on the selection, whether a selection could
//! still grow into what it needs, and putting it there. Their names, hints and glyphs are in
//! `constrain_text.rs`, the clicks of a constraint tool in `constrain_pick.rs`.

use super::*;
use simple3d_sketch::{ConstraintKind, Shape};

/// A geometric constraint, as a tool. Dimensions are the Dimension tool's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Rule {
    /// Two points together, or a point on a line, arc, circle or ellipse.
    Coincident,
    Midpoint,
    Horizontal,
    Vertical,
    Parallel,
    Perpendicular,
    Tangent,
    Equal,
    Symmetric,
    Fix,
}

/// What a selection is made of, by the kinds a rule tells apart.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Counts {
    pub points: usize,
    pub lines: usize,
    /// Arcs and circles.
    pub rounds: usize,
    /// Ellipses and elliptical arcs.
    pub elliptic: usize,
    /// Splines.
    pub splines: usize,
    pub constraints: usize,
}

impl Counts {
    pub(crate) fn curves(&self) -> usize {
        self.lines + self.rounds + self.elliptic + self.splines
    }
}

impl SketchTool {
    pub(crate) fn selected_points(&self) -> Vec<PointId> {
        self.selection.iter().filter_map(|p| if let Pick::Point(id) = p { Some(*id) } else { None }).collect()
    }

    pub(crate) fn selected_curves(&self) -> Vec<CurveId> {
        self.selection.iter().filter_map(|p| if let Pick::Curve(id) = p { Some(*id) } else { None }).collect()
    }

    pub(crate) fn shape(&self, id: CurveId) -> Option<&Shape> {
        self.sketch.curve(id).map(|c| &c.shape)
    }

    /// A line's ends.
    pub(crate) fn ends(&self, id: CurveId) -> Option<(PointId, PointId)> {
        match self.shape(id)? {
            Shape::Line { a, b } => Some((*a, *b)),
            _ => None,
        }
    }

    /// The selection by kind.
    pub(crate) fn counts(&self) -> Counts {
        let mut counts = Counts::default();
        for pick in &self.selection {
            match *pick {
                Pick::Point(_) => counts.points += 1,
                Pick::Constraint(_) => counts.constraints += 1,
                Pick::Curve(id) => match self.shape(id) {
                    Some(shape) if shape.is_line() => counts.lines += 1,
                    Some(shape) if shape.is_round() => counts.rounds += 1,
                    Some(shape) if shape.is_elliptic() => counts.elliptic += 1,
                    _ => counts.splines += 1,
                },
            }
        }
        counts
    }

    /// Whether more picks could still make the selection what `rule` needs: what it has so far is no
    /// more than one of the sets it takes.
    pub(crate) fn could_fit(&self, rule: Rule) -> bool {
        let n = self.counts();
        let others = n.rounds + n.elliptic + n.splines;
        n.constraints == 0
            && match rule {
                Rule::Coincident => n.splines == 0 && n.curves() <= 1 && n.points + n.curves() <= 2,
                Rule::Midpoint => n.points <= 1 && n.lines <= 1 && others == 0,
                Rule::Horizontal | Rule::Vertical => n.points == 0 && others == 0,
                Rule::Parallel | Rule::Perpendicular => n.points == 0 && others == 0 && n.lines <= 2,
                Rule::Tangent => n.points == 0 && n.curves() <= 2,
                Rule::Equal => n.points == 0 && n.elliptic + n.splines == 0 && (n.lines == 0 || n.rounds == 0),
                Rule::Symmetric => n.points <= 2 && n.lines <= 1 && others == 0,
                Rule::Fix => true,
            }
    }

    /// What `rule` would put on the selection as it is, or `None` when it does not fit.
    pub(crate) fn kinds_for(&self, rule: Rule) -> Option<Vec<ConstraintKind>> {
        use ConstraintKind as K;
        if !self.could_fit(rule) {
            return None;
        }
        let (points, curves) = (self.selected_points(), self.selected_curves());
        let lines: Vec<CurveId> = curves.iter().copied().filter(|&c| self.ends(c).is_some()).collect();
        let n = self.counts();
        let kinds = match (rule, points.len(), curves.len()) {
            (Rule::Coincident, 2, 0) => vec![K::Coincident { a: points[0], b: points[1] }],
            (Rule::Coincident, 1, 1) => vec![K::OnCurve { point: points[0], curve: curves[0] }],
            (Rule::Horizontal, 0, _) => lines.iter().map(|&line| K::Horizontal { line }).collect(),
            (Rule::Vertical, 0, _) => lines.iter().map(|&line| K::Vertical { line }).collect(),
            (Rule::Parallel, 0, 2) => vec![K::Parallel { a: lines[0], b: lines[1] }],
            (Rule::Perpendicular, 0, 2) => vec![K::Perpendicular { a: lines[0], b: lines[1] }],
            (Rule::Tangent, 0, 2) => {
                let inside = match (self.round_of(curves[0]), self.round_of(curves[1])) {
                    (Some((c1, r1)), Some((c2, r2))) => (c1[0] - c2[0]).hypot(c1[1] - c2[1]) < r1.max(r2),
                    _ => false,
                };
                vec![K::Tangent { a: curves[0], b: curves[1], inside }]
            }
            (Rule::Equal, 0, k) if k >= 2 && (n.lines == k || n.rounds == k) => {
                curves[1..].iter().map(|&b| K::Equal { a: curves[0], b }).collect()
            }
            (Rule::Symmetric, 2, 1) => vec![K::Symmetric { a: points[0], b: points[1], line: lines[0] }],
            (Rule::Midpoint, 1, 1) => vec![K::Midpoint { point: points[0], line: lines[0] }],
            (Rule::Fix, _, _) if !self.selection.is_empty() => {
                let mut ids = points.clone();
                ids.extend(curves.iter().flat_map(|&c| self.shape(c).map(Shape::points).unwrap_or_default()));
                ids.sort_unstable();
                ids.dedup();
                ids.into_iter()
                    .filter(|&p| self.sketch.point(p).is_some_and(|p| !p.fixed))
                    .map(|point| K::Fix { point, at: self.sketch.at(point) })
                    .collect()
            }
            _ => return None,
        };
        (!kinds.is_empty()).then_some(kinds)
    }

    /// A round curve's centre and radius.
    pub(crate) fn round_of(&self, id: CurveId) -> Option<([f64; 2], f64)> {
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
        let mut added = 0;
        for kind in kinds {
            match self.sketch.constrain(kind) {
                Ok(_) => added += 1,
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
        Ok(added)
    }
}

pub(crate) fn gap(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
