//! Where the cells sit: centring, extent, and which ones a body reaches.

use super::*;
use crate::vec3::Vec3;

/// How far the lattice shifts to centre the cells on a span: half a cell for an even count, none
/// for odd.
pub(crate) fn centring(span: f64, step: f64) -> f64 {
    let count = (span / step).ceil().max(1.0) as i64;
    if count % 2 == 0 {
        step / 2.0
    } else {
        0.0
    }
}

/// The lattice rows or columns reaching a span, with a cell of margin each side.
pub(crate) fn range(from: f64, to: f64, step: f64) -> std::ops::RangeInclusive<i64> {
    let first = (from / step).floor() as i64 - 1;
    let last = (to / step).ceil() as i64 + 1;
    first..=last
}

/// Each layer's start and end along the axis; the outer ones overshoot so end pieces are capped by
/// the shape's own surface.
pub(crate) fn layers(tiling: &Tiling, lo: f64, hi: f64) -> Vec<(f64, f64)> {
    if tiling.layer < MIN_SIZE {
        return vec![(lo - OVERSHOOT, hi + OVERSHOOT)];
    }
    let count = tiling.layer_count(hi - lo);
    (0..count)
        .map(|k| {
            let from = if k == 0 { lo - OVERSHOOT } else { lo + k as f64 * tiling.layer };
            let to = if k == count - 1 { hi + OVERSHOOT } else { lo + (k + 1) as f64 * tiling.layer };
            (from, to)
        })
        .collect()
}

/// Whether a cell can hold any of a shape, stricter than [`crate::boxes_overlap`], which counts
/// touching boxes: a cell flush against the shape holds nothing.
pub(crate) fn reaches(cell: (Vec3, Vec3), shape: (Vec3, Vec3)) -> bool {
    const EPS: f64 = 1e-6;
    let ((clo, chi), (slo, shi)) = (cell, shape);
    clo.x + EPS < shi.x
        && slo.x + EPS < chi.x
        && clo.y + EPS < shi.y
        && slo.y + EPS < chi.y
        && clo.z + EPS < shi.z
        && slo.z + EPS < chi.z
}

pub(crate) fn bound(tiling: &Tiling, centre: (f64, f64), outline: Vec<(f64, f64)>, span: (f64, f64)) -> Cell {
    let mut lo = tiling.to_world(outline[0].0, outline[0].1, span.0);
    let mut hi = lo;
    for &(x, y) in &outline {
        for w in [span.0, span.1] {
            let p = tiling.to_world(x, y, w);
            lo = lo.min(p);
            hi = hi.max(p);
        }
    }
    Cell { outline, centre, span, bounds: (lo, hi) }
}

/// How many cells could actually hold a piece, the count worth showing and measuring progress by.
/// [`Tiling::cell_count`] is the cheap upper bound, checked first.
pub fn planned(tiling: &Tiling, bounds: (Vec3, Vec3)) -> usize {
    if tiling.refusal(bounds).is_some() {
        return 0;
    }
    plan(tiling, bounds).len()
}
