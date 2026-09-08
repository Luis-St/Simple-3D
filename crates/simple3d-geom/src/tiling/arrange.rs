//! Where the cells sit: centring, extent, and which of them a body reaches.

use super::*;
use crate::vec3::Vec3;

/// How far the lattice is stood over so that the cells across a span are
/// centred on it: half a cell where an even number of them covers it, nothing
/// where an odd number does and the middle cell already sits in the middle.
pub(crate) fn centring(span: f64, step: f64) -> f64 {
    let count = (span / step).ceil().max(1.0) as i64;
    if count % 2 == 0 {
        step / 2.0
    } else {
        0.0
    }
}

/// The indices of the lattice rows or columns that can reach a span, with a
/// cell of margin on each side so nothing at the edge is missed.
pub(crate) fn range(from: f64, to: f64, step: f64) -> std::ops::RangeInclusive<i64> {
    let first = (from / step).floor() as i64 - 1;
    let last = (to / step).ceil() as i64 + 1;
    first..=last
}

/// Where each layer of cells starts and ends along the axis. The first and the
/// last overshoot the shape, so the pieces there are capped by the shape's own
/// surface rather than by the cell.
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

/// Whether a cell can hold any of a shape, which is a stricter question than
/// whether their boxes overlap.
///
/// [`crate::boxes_overlap`] answers yes for boxes that merely *touch*, because
/// two coincident faces are exactly what the boolean kernel must be given a
/// chance to handle. Here the opposite is wanted: a cell sitting exactly against
/// the side of the shape holds a slice of it nothing thick, which is not a
/// piece, and counting it says a number nobody can find in the outliner.
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

/// How many cells the tiling actually lays over a shape of these bounds -- the
/// ones that could hold a piece of it, which is what a count worth showing
/// means and what the progress of a split is measured against.
///
/// [`Tiling::cell_count`] is the arithmetic upper bound and is what refuses a
/// number too large to build; this one plans the cells to find out, so it is
/// only asked once the count is known to be sane.
pub fn planned(tiling: &Tiling, bounds: (Vec3, Vec3)) -> usize {
    if tiling.refusal(bounds).is_some() {
        return 0;
    }
    plan(tiling, bounds).len()
}
