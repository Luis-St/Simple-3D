//! Push/pull (issue 73): a flat region of a mesh as an outline, and an outline extruded into a solid.
//!
//! The outline is captured once, in a frame on the face, and stored; it is not re-derived from the model
//! afterwards, so the solid stays put when the faces around it change.

mod region;
pub use region::{flat_face, FlatFace, CURVED_DEG};
mod extrude;
pub use extrude::extrude_outline;
#[cfg(test)]
mod tests;

use crate::vec3::Vec3;
use serde::{Deserialize, Serialize};

/// Two axes spanning a plane with normal `normal`, with `u x v` along it: a frame to capture a face
/// in when nothing better is known.
pub fn face_basis(normal: Vec3) -> (Vec3, Vec3) {
    crate::planar::plane_basis(normal)
}

/// A closed planar profile with holes, in its own 2D frame: the outer loop counter-clockwise, every
/// hole clockwise, each without repeated or collinear points.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Outline {
    pub outer: Vec<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub holes: Vec<Vec<[f64; 2]>>,
}

impl Outline {
    /// An outline from loops of either winding, cleaned up and wound as the type promises. `None`
    /// when the outer loop has no area left.
    pub fn new(outer: Vec<[f64; 2]>, holes: Vec<Vec<[f64; 2]>>) -> Option<Outline> {
        let outer = wound(clean(outer)?, true);
        let holes = holes.into_iter().filter_map(clean).map(|hole| wound(hole, false)).collect();
        Some(Outline { outer, holes })
    }

    /// The outline's box, as (min, max) corners.
    pub fn bounds(&self) -> ([f64; 2], [f64; 2]) {
        let mut lo = [f64::INFINITY; 2];
        let mut hi = [f64::NEG_INFINITY; 2];
        for p in &self.outer {
            for axis in 0..2 {
                lo[axis] = lo[axis].min(p[axis]);
                hi[axis] = hi[axis].max(p[axis]);
            }
        }
        (lo, hi)
    }

    /// The enclosed area, holes taken off.
    pub fn area(&self) -> f64 {
        area(&self.outer) + self.holes.iter().map(|hole| area(hole)).sum::<f64>()
    }
}

/// The signed area by the shoelace formula, positive counter-clockwise.
fn area(points: &[[f64; 2]]) -> f64 {
    let n = points.len();
    (0..n)
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % n]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>()
        * 0.5
}

fn wound(mut points: Vec<[f64; 2]>, counter_clockwise: bool) -> Vec<[f64; 2]> {
    if (area(&points) > 0.0) != counter_clockwise {
        points.reverse();
    }
    points
}

/// The loop without repeated points or points on the line between their neighbours, which would
/// leave T-junctions between the walls and the caps. `None` below three corners or with no area.
fn clean(points: Vec<[f64; 2]>) -> Option<Vec<[f64; 2]>> {
    let mut points: Vec<[f64; 2]> = points.into_iter().filter(|p| p[0].is_finite() && p[1].is_finite()).collect();
    loop {
        let n = points.len();
        if n < 3 {
            return None;
        }
        let keep: Vec<bool> = (0..n)
            .map(|i| {
                let (a, b, c) = (points[(i + n - 1) % n], points[i], points[(i + 1) % n]);
                let cross = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
                let span = ((c[0] - a[0]).powi(2) + (c[1] - a[1]).powi(2)).sqrt();
                let apart = (b[0] - a[0]).abs() + (b[1] - a[1]).abs() > 1e-9;
                apart && span > 1e-12 && cross.abs() / span > 1e-7
            })
            .collect();
        if keep.iter().all(|&k| k) {
            break;
        }
        let mut index = 0;
        points.retain(|_| {
            index += 1;
            keep[index - 1]
        });
    }
    (area(&points).abs() > 1e-9).then_some(points)
}
