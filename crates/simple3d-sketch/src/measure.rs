//! Dimensions as measured on the sketch as it stands, and reference dimensions: dimensions that only
//! measure, following the sketch instead of driving it, as CAD programs keep a size on show that the
//! other constraints already decide.

use crate::constraint::{ConstraintId, ConstraintKind};
use crate::curve::Shape;
use crate::sketch::Sketch;
use crate::solver::{angle_between, Refusal};
use crate::{cross, distance, sub};

impl ConstraintKind {
    /// What a dimension of this kind measures on `sketch` now, as its value would be written; `None`
    /// for a plain geometric constraint or one naming what is not there.
    pub fn measure(&self, sketch: &Sketch) -> Option<f64> {
        use ConstraintKind::*;
        let at = |p| sketch.point(p).map(|p| p.at);
        let line = |id| match sketch.curve(id)?.shape {
            Shape::Line { a, b } => Some((at(a)?, at(b)?)),
            _ => None,
        };
        let radius = |id| match sketch.curve(id)?.shape {
            Shape::Circle { radius, .. } => Some(radius),
            Shape::Arc { center, start, .. } => Some(distance(at(center)?, at(start)?)),
            _ => None,
        };
        Some(match *self {
            Distance { a, b, .. } => distance(at(a)?, at(b)?),
            HorizontalDistance { a, b, .. } => (at(b)?[0] - at(a)?[0]).abs(),
            VerticalDistance { a, b, .. } => (at(b)?[1] - at(a)?[1]).abs(),
            PointLineDistance { point, line: l, .. } => {
                let ((p, q), r) = (line(l)?, at(point)?);
                cross(sub(q, p), sub(r, p)).abs() / distance(p, q).max(1e-12)
            }
            Radius { curve, .. } => radius(curve)?,
            Diameter { curve, .. } => 2.0 * radius(curve)?,
            Angle { a, b, .. } => {
                let ((p, q), (r, t)) = (line(a)?, line(b)?);
                angle_between(sub(q, p), sub(t, r)).to_degrees()
            }
            _ => return None,
        })
    }
}

impl Sketch {
    /// Bring every reference dimension's value up to what it measures.
    pub(crate) fn measure_driven(&mut self) {
        let measured: Vec<(usize, f64)> = self
            .constraints
            .iter()
            .enumerate()
            .filter(|(_, c)| c.driven)
            .filter_map(|(i, c)| Some((i, c.kind.measure(self)?)))
            .collect();
        for (i, value) in measured {
            if let Some(slot) = self.constraints[i].kind.value_mut() {
                *slot = value;
            }
        }
    }

    /// Add a dimension as a reference dimension, measuring what it names; nothing moves.
    pub fn add_reference(&mut self, kind: ConstraintKind) -> Result<ConstraintId, Refusal> {
        if kind.value().is_none() {
            return Err(Refusal::Unsupported("Only a dimension can be a reference dimension"));
        }
        let id = self.push_constraint(kind);
        if let Some(constraint) = self.constraint_mut(id) {
            constraint.driven = true;
        }
        self.measure_driven();
        Ok(id)
    }

    /// Make dimension `id` a reference dimension or a driving one again; a driving one is refused,
    /// with nothing changed, when the other constraints already decide what it measures.
    pub fn set_driven(&mut self, id: ConstraintId, driven: bool) -> Result<(), Refusal> {
        let before = self.clone();
        let Some(constraint) = self.constraint_mut(id).filter(|c| c.kind.value().is_some()) else {
            return Err(Refusal::Unsupported("Only a dimension can be a reference dimension"));
        };
        constraint.driven = driven;
        if driven {
            self.measure_driven();
            return Ok(());
        }
        if let Some(others) = self.repeated_by(id) {
            *self = before;
            return Err(Refusal::Redundant(others));
        }
        if !self.solve() {
            *self = before;
            return Err(Refusal::Unsolvable);
        }
        Ok(())
    }
}
