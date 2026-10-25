//! A sketch as unknowns and equations: every free point's X and Y and every circle's radius, and
//! one equation per thing that must hold.

use crate::constraint::ConstraintId;
use crate::curve::{CurveId, Shape};
use crate::point::PointId;
use crate::sketch::Sketch;
use std::collections::HashMap;

/// An equation's value at the unknowns, zero when it holds.
pub(crate) type Residual<'a> = Box<dyn Fn(&System, &[f64]) -> f64 + 'a>;

/// One equation, zero when it holds: from a constraint, or `None` for what a curve demands of
/// itself (an arc's end on its circle).
pub(crate) struct Equation<'a> {
    pub owner: Option<ConstraintId>,
    /// The unknowns it reads, so the Jacobian is only differentiated where it can be non-zero.
    pub deps: Vec<usize>,
    pub eval: Residual<'a>,
}

pub(crate) struct System<'a> {
    pub sketch: &'a Sketch,
    /// Where each free point's X sits among the unknowns; its Y follows.
    pub points: HashMap<PointId, usize>,
    pub radii: HashMap<CurveId, usize>,
    /// The unknowns' values as the solve began.
    pub start: Vec<f64>,
}

impl<'a> System<'a> {
    pub fn new(sketch: &'a Sketch) -> System<'a> {
        let mut start = Vec::new();
        let mut points = HashMap::new();
        for point in sketch.points.iter().filter(|p| !p.fixed) {
            points.insert(point.id, start.len());
            start.extend(point.at);
        }
        let mut radii = HashMap::new();
        for curve in &sketch.curves {
            if let Shape::Circle { radius, .. } = curve.shape {
                radii.insert(curve.id, start.len());
                start.push(radius);
            }
        }
        System { sketch, points, radii, start }
    }

    pub fn len(&self) -> usize {
        self.start.len()
    }

    /// Point `id` at unknowns `x`; a fixed point where it is stored.
    pub fn pos(&self, x: &[f64], id: PointId) -> [f64; 2] {
        match self.points.get(&id) {
            Some(&i) => [x[i], x[i + 1]],
            None => self.sketch.at(id),
        }
    }

    /// The unknowns a point is made of; none for a fixed point.
    pub fn point_vars(&self, id: PointId) -> Vec<usize> {
        self.points.get(&id).map_or(Vec::new(), |&i| vec![i, i + 1])
    }

    /// The unknowns a curve is made of: its points and a circle's radius.
    pub fn curve_vars(&self, id: CurveId) -> Vec<usize> {
        let Some(curve) = self.sketch.curve(id) else { return Vec::new() };
        let mut vars: Vec<usize> = curve.shape.points().into_iter().flat_map(|p| self.point_vars(p)).collect();
        vars.extend(self.radii.get(&id));
        vars
    }

    /// A line's ends at `x`.
    pub fn line(&self, x: &[f64], id: CurveId) -> Option<([f64; 2], [f64; 2])> {
        match self.sketch.curve(id)?.shape {
            Shape::Line { a, b } => Some((self.pos(x, a), self.pos(x, b))),
            _ => None,
        }
    }

    /// A round curve's centre and radius at `x`.
    pub fn round(&self, x: &[f64], id: CurveId) -> Option<([f64; 2], f64)> {
        match self.sketch.curve(id)?.shape {
            Shape::Circle { center, .. } => Some((self.pos(x, center), x[self.radii[&id]])),
            Shape::Arc { center, start, .. } => {
                let c = self.pos(x, center);
                Some((c, crate::distance(self.pos(x, start), c)))
            }
            _ => None,
        }
    }

    /// The direction curve `id` runs in at its end point `at`, following the curve's own travel, so
    /// two curves meeting there smoothly have parallel (or opposite) directions. `None` when `at` is
    /// not an end of it.
    pub fn end_direction(&self, x: &[f64], id: CurveId, at: PointId) -> Option<[f64; 2]> {
        let shape = &self.sketch.curve(id)?.shape;
        let (first, last) = shape.ends()?;
        let toward = |from: PointId, to: PointId| crate::sub(self.pos(x, to), self.pos(x, from));
        match shape {
            Shape::Line { a, b } => Some(toward(*a, *b)),
            Shape::Arc { center, .. } => {
                let r = toward(*center, at);
                // Counter-clockwise travel: the tangent is the radius turned a quarter left.
                Some([-r[1], r[0]])
            }
            Shape::Spline { points, .. } | Shape::Bspline { points, .. } => {
                let n = points.len();
                if at == first {
                    Some(toward(points[0], points[1]))
                } else if at == last {
                    Some(toward(points[n - 2], points[n - 1]))
                } else {
                    None
                }
            }
            Shape::Circle { .. } => None,
        }
        .filter(|_| at == first || at == last)
    }

    /// The equations of the whole sketch: each arc's own, then every constraint's in order.
    pub fn equations(&self) -> Result<Vec<Equation<'a>>, (ConstraintId, &'static str)> {
        let mut out = Vec::new();
        for curve in &self.sketch.curves {
            if let Shape::Arc { center, start, end } = curve.shape {
                let mut deps = self.point_vars(center);
                deps.extend(self.point_vars(start));
                deps.extend(self.point_vars(end));
                out.push(Equation {
                    owner: None,
                    deps,
                    eval: Box::new(move |s: &System, x: &[f64]| {
                        let c = s.pos(x, center);
                        crate::distance(s.pos(x, end), c) - crate::distance(s.pos(x, start), c)
                    }),
                });
            }
        }
        for constraint in &self.sketch.constraints {
            let equations = super::equations::of(self, &constraint.kind).map_err(|why| (constraint.id, why))?;
            out.extend(equations.into_iter().map(|(deps, eval)| Equation { owner: Some(constraint.id), deps, eval }));
        }
        Ok(out)
    }

    /// Write unknowns `x` back into a copy of the sketch.
    pub fn write(&self, x: &[f64], into: &mut Sketch) {
        for point in &mut into.points {
            if let Some(&i) = self.points.get(&point.id) {
                point.at = [x[i], x[i + 1]];
            }
        }
        for curve in &mut into.curves {
            if let (Shape::Circle { radius, .. }, Some(&i)) = (&mut curve.shape, self.radii.get(&curve.id)) {
                *radius = x[i];
            }
        }
    }
}
