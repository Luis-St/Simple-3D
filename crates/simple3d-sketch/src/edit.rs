//! Adding and removing what a sketch is drawn from. Removing takes along everything that refers to
//! what goes, so the sketch never names something it does not hold.

use crate::constraint::{Constraint, ConstraintId, ConstraintKind};
use crate::curve::{Curve, CurveId, Shape};
use crate::point::{Point, PointId};
use crate::sketch::Sketch;

impl Sketch {
    fn next_point_id(&self) -> PointId {
        self.points.iter().map(|p| p.id).max().map_or(1, |most| most + 1)
    }

    fn next_curve_id(&self) -> CurveId {
        self.curves.iter().map(|c| c.id).max().map_or(1, |most| most + 1)
    }

    fn next_constraint_id(&self) -> ConstraintId {
        self.constraints.iter().map(|c| c.id).max().map_or(1, |most| most + 1)
    }

    /// A new free point at `at`.
    pub fn add_point(&mut self, at: [f64; 2]) -> PointId {
        let id = self.next_point_id();
        self.points.push(Point { id, at, fixed: false });
        id
    }

    /// A new curve; its points must already be in the sketch.
    pub fn add_curve(&mut self, shape: Shape, construction: bool) -> CurveId {
        let id = self.next_curve_id();
        self.curves.push(Curve { id, shape, construction, reference: false });
        id
    }

    /// A line between two new points, or existing ones where given.
    pub fn add_line(&mut self, a: [f64; 2], b: [f64; 2]) -> CurveId {
        let (a, b) = (self.add_point(a), self.add_point(b));
        self.add_curve(Shape::Line { a, b }, false)
    }

    /// A circle about a new centre point.
    pub fn add_circle(&mut self, center: [f64; 2], radius: f64) -> CurveId {
        let center = self.add_point(center);
        self.add_curve(Shape::Circle { center, radius }, false)
    }

    /// A fixed line copied in from the model, from `a` to `b`, reusing a fixed point already at
    /// either end so copied edges stay joined.
    pub fn add_reference_line(&mut self, a: [f64; 2], b: [f64; 2], construction: bool) -> Option<CurveId> {
        if crate::distance(a, b) < crate::EPS {
            return None;
        }
        let mut fixed_at = |at: [f64; 2]| -> PointId {
            match self.points.iter().find(|p| p.fixed && crate::distance(p.at, at) < crate::EPS) {
                Some(p) => p.id,
                None => {
                    let id = self.add_point(at);
                    self.point_mut(id).expect("just added").fixed = true;
                    id
                }
            }
        };
        let (a, b) = (fixed_at(a), fixed_at(b));
        let id = self.add_curve(Shape::Line { a, b }, construction);
        self.curve_mut(id).expect("just added").reference = true;
        Some(id)
    }

    /// Add a constraint without checking it; see [`Sketch::constrain`] for the checked way.
    pub fn push_constraint(&mut self, kind: ConstraintKind) -> ConstraintId {
        let id = self.next_constraint_id();
        self.constraints.push(Constraint { id, kind });
        id
    }

    pub fn remove_constraint(&mut self, id: ConstraintId) {
        self.constraints.retain(|c| c.id != id);
    }

    /// Remove a curve with the constraints on it, the region picks along it and any point nothing
    /// else uses any more.
    pub fn remove_curve(&mut self, id: CurveId) {
        let Some(curve) = self.curve(id).cloned() else { return };
        self.curves.retain(|c| c.id != id);
        self.constraints.retain(|c| !c.kind.curves().contains(&id));
        self.regions.retain(|pick| pick.sides.iter().all(|&(curve, _)| curve != id));
        if self.axis == Some(id) {
            self.axis = None;
        }
        for point in curve.shape.points() {
            if !self.point_used(point) {
                self.remove_point(point);
            }
        }
    }

    /// Remove a point with every curve drawn from it and every constraint on it.
    pub fn remove_point(&mut self, id: PointId) {
        if self.point(id).is_none() {
            return;
        }
        self.points.retain(|p| p.id != id);
        self.constraints.retain(|c| !c.kind.points().contains(&id));
        let curves: Vec<CurveId> =
            self.curves.iter().filter(|c| c.shape.points().contains(&id)).map(|c| c.id).collect();
        for curve in curves {
            self.remove_curve(curve);
        }
    }

    /// Whether any curve is drawn from point `id`.
    pub fn point_used(&self, id: PointId) -> bool {
        self.curves.iter().any(|c| c.shape.points().contains(&id))
    }

    /// Fold point `gone` into `kept`: every curve and constraint naming it names `kept` instead.
    /// How drawing onto an existing point joins to it without a coincident constraint.
    pub fn merge_points(&mut self, kept: PointId, gone: PointId) {
        if kept == gone || self.point(kept).is_none() {
            return;
        }
        let swap = |p: &mut PointId| {
            if *p == gone {
                *p = kept;
            }
        };
        for curve in &mut self.curves {
            match &mut curve.shape {
                Shape::Line { a, b } => [a, b].into_iter().for_each(swap),
                Shape::Arc { center, start, end } => [center, start, end].into_iter().for_each(swap),
                Shape::Circle { center, .. } => swap(center),
                Shape::Spline { points, .. } | Shape::Bspline { points, .. } => points.iter_mut().for_each(swap),
            }
        }
        for constraint in &mut self.constraints {
            use ConstraintKind::*;
            match &mut constraint.kind {
                Coincident { a, b }
                | Distance { a, b, .. }
                | HorizontalDistance { a, b, .. }
                | VerticalDistance { a, b, .. }
                | Symmetric { a, b, .. } => [a, b].into_iter().for_each(swap),
                Midpoint { point, .. }
                | OnCurve { point, .. }
                | PointLineDistance { point, .. }
                | Fix { point, .. } => swap(point),
                _ => {}
            }
        }
        // A coincidence of the point with itself says nothing, and a line from it to itself is nothing.
        self.constraints.retain(|c| !matches!(c.kind, ConstraintKind::Coincident { a, b } if a == b));
        let degenerate: Vec<CurveId> =
            self.curves.iter().filter(|c| matches!(c.shape, Shape::Line { a, b } if a == b)).map(|c| c.id).collect();
        self.points.retain(|p| p.id != gone);
        for curve in degenerate {
            self.remove_curve(curve);
        }
    }
}
