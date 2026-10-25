//! Sketches (issue 74): a 2D profile drawn from points and curves, held in shape by geometric
//! constraints and dimensions, and turned into a solid by extruding or revolving the regions picked
//! out of it.
//!
//! The pieces stand on each other in this order:
//!
//! * [`Sketch`] is the stored drawing: points, curves (lines, arcs, circles and two kinds of
//!   spline), constraints, the picked regions and the revolve axis. Everything refers to everything
//!   else by id, so deleting a point takes what hangs off it along.
//! * [`solver`] moves the points until every constraint holds, keeping them as close to where they
//!   were as it can, and says which points are still free and which constraints fight each other.
//! * [`regions`] breaks the plane up wherever curves cross and finds every closed area, which is what
//!   the user picks from. A pick is remembered by the curves around it, so it survives edits.
//! * [`feature`] turns the picked regions into a mesh: extruded to one side, both or symmetrically,
//!   with a draft, or revolved about a line of the sketch.

mod constraint;
pub use constraint::{Constraint, ConstraintId, ConstraintKind};
mod curve;
pub use curve::{Curve, CurveId, Shape};
mod edit;
mod point;
pub use point::{Point, PointId};
mod sample;
pub use sample::{sample_curve, Samples};
mod sketch;
pub use sketch::{RegionPick, Sketch};
pub mod feature;
pub mod regions;
pub mod solver;

/// Two points this close are the same point, in millimetres: well under anything a printer or a
/// typed dimension can tell apart, and well over what the solver leaves behind.
pub const EPS: f64 = 1e-6;

pub(crate) fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

pub(crate) fn add(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] + b[0], a[1] + b[1]]
}

pub(crate) fn scale(a: [f64; 2], s: f64) -> [f64; 2] {
    [a[0] * s, a[1] * s]
}

pub(crate) fn dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

pub(crate) fn cross(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

pub(crate) fn length(a: [f64; 2]) -> f64 {
    a[0].hypot(a[1])
}

pub(crate) fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    length(sub(a, b))
}

#[cfg(test)]
mod tests;
