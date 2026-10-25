//! The stored drawing.

use crate::constraint::{Constraint, ConstraintId};
use crate::curve::{Curve, CurveId};
use crate::point::{Point, PointId};
use serde::{Deserialize, Serialize};

/// A sketch: everything drawn and constrained, the regions picked to become a solid, and the line a
/// revolve turns about. Lives in the sketch node's own XY plane.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Sketch {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<Point>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub curves: Vec<Curve>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<Constraint>,
    /// The regions picked to extrude or revolve.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regions: Vec<RegionPick>,
    /// The line a revolve turns the regions about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub axis: Option<CurveId>,
}

/// A picked region, remembered by the sides of the curves around it rather than by number, since
/// numbers change with every edit while the curves stay. `hint` is a point inside it when picked,
/// which settles a tie between two regions with the same curves around them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RegionPick {
    /// Each curve along its edge, with whether the region lies on the curve's left as it runs from
    /// its start to its end. Sorted, without repeats.
    pub sides: Vec<(CurveId, bool)>,
    pub hint: [f64; 2],
}

impl Sketch {
    pub fn point(&self, id: PointId) -> Option<&Point> {
        self.points.iter().find(|p| p.id == id)
    }

    pub fn point_mut(&mut self, id: PointId) -> Option<&mut Point> {
        self.points.iter_mut().find(|p| p.id == id)
    }

    /// Where point `id` is; the origin for a point that is not there, so a broken file draws
    /// something rather than panicking.
    pub fn at(&self, id: PointId) -> [f64; 2] {
        self.point(id).map_or([0.0; 2], |p| p.at)
    }

    pub fn curve(&self, id: CurveId) -> Option<&Curve> {
        self.curves.iter().find(|c| c.id == id)
    }

    pub fn curve_mut(&mut self, id: CurveId) -> Option<&mut Curve> {
        self.curves.iter_mut().find(|c| c.id == id)
    }

    pub fn constraint(&self, id: ConstraintId) -> Option<&Constraint> {
        self.constraints.iter().find(|c| c.id == id)
    }

    pub fn constraint_mut(&mut self, id: ConstraintId) -> Option<&mut Constraint> {
        self.constraints.iter_mut().find(|c| c.id == id)
    }

    /// Whether nothing at all is drawn.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty() && self.curves.is_empty()
    }

    /// The box around every point and circle, as (min, max) corners; `None` when empty.
    pub fn bounds(&self) -> Option<([f64; 2], [f64; 2])> {
        let mut lo = [f64::INFINITY; 2];
        let mut hi = [f64::NEG_INFINITY; 2];
        let mut grow = |p: [f64; 2], r: f64| {
            for axis in 0..2 {
                lo[axis] = lo[axis].min(p[axis] - r);
                hi[axis] = hi[axis].max(p[axis] + r);
            }
        };
        for point in &self.points {
            grow(point.at, 0.0);
        }
        for curve in &self.curves {
            if let crate::Shape::Circle { center, radius } = curve.shape {
                grow(self.at(center), radius.abs());
            }
        }
        (lo[0] <= hi[0]).then_some((lo, hi))
    }

    /// Whether the sketch refers only to things it holds: every curve's points, every constraint's
    /// points and curves, and the axis. A file that fails this is refused rather than half-drawn.
    pub fn is_consistent(&self) -> bool {
        let has_point = |id: PointId| self.point(id).is_some();
        let has_curve = |id: CurveId| self.curve(id).is_some();
        let curves_ok = self.curves.iter().all(|c| {
            let points = c.shape.points();
            let enough = match &c.shape {
                crate::Shape::Spline { points, closed } | crate::Shape::Bspline { points, closed } => {
                    points.len() >= if *closed { 3 } else { 2 }
                }
                _ => true,
            };
            enough && points.iter().all(|&p| has_point(p))
        });
        let constraints_ok = self.constraints.iter().all(|c| {
            c.kind.points().iter().all(|&p| has_point(p)) && c.kind.curves().iter().all(|&k| has_curve(k))
        });
        curves_ok && constraints_ok && self.axis.is_none_or(has_curve)
    }
}
