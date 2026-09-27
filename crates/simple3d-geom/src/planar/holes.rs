//! Bridging a hole into its surrounding loop, so a region with holes ear-clips as one polygon.

use super::*;
pub(crate) fn point_in_polygon(p: Point, polygon: &[Point]) -> bool {
    let n = polygon.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (polygon[i], polygon[(i + 1) % n]);
        if (a.1 > p.1) != (b.1 > p.1) {
            let x = a.0 + (p.1 - a.1) / (b.1 - a.1) * (b.0 - a.0);
            if x > p.0 {
                inside = !inside;
            }
        }
    }
    inside
}

/// Splice `hole` into `outer` along a bridge from the hole's rightmost vertex to the nearest outer
/// vertex reachable without crossing either loop or the `waiting` holes. Both endpoints appear twice,
/// a zero-width seam. Ignoring waiting holes once let a bridge cross them in a drilled grid.
pub(crate) fn bridge_hole(
    outer: &mut Vec<u32>,
    outer_points: &mut Vec<Point>,
    hole: &[u32],
    hole_points: &[Point],
    waiting: &[&[Point]],
) -> Option<()> {
    let start = (0..hole_points.len())
        .max_by(|&a, &b| hole_points[a].0.partial_cmp(&hole_points[b].0).unwrap_or(std::cmp::Ordering::Equal))?;
    let from = hole_points[start];

    let mut candidates: Vec<usize> = (0..outer_points.len()).collect();
    candidates.sort_by(|&a, &b| {
        let d = |i: usize| {
            let p = outer_points[i];
            (p.0 - from.0).powi(2) + (p.1 - from.1).powi(2)
        };
        d(a).partial_cmp(&d(b)).unwrap_or(std::cmp::Ordering::Equal)
    });

    let target = candidates.into_iter().find(|&c| {
        let to = outer_points[c];
        !crosses_any(from, to, outer_points)
            && !crosses_any(from, to, hole_points)
            && waiting.iter().all(|other| !crosses_any(from, to, other))
    })?;

    // outer[..=target] + hole from `start` all the way round + hole[start] + outer[target..]
    let mut ids = Vec::with_capacity(outer.len() + hole.len() + 2);
    let mut points = Vec::with_capacity(ids.capacity());
    for i in 0..=target {
        ids.push(outer[i]);
        points.push(outer_points[i]);
    }
    for k in 0..=hole.len() {
        let i = (start + k) % hole.len();
        ids.push(hole[i]);
        points.push(hole_points[i]);
    }
    for i in target..outer.len() {
        ids.push(outer[i]);
        points.push(outer_points[i]);
    }
    *outer = ids;
    *outer_points = points;
    Some(())
}

/// Whether open segment `a`-`b` properly crosses an edge of `polygon`; endpoint contact does not count.
pub(crate) fn crosses_any(a: Point, b: Point, polygon: &[Point]) -> bool {
    let n = polygon.len();
    (0..n).any(|i| segments_properly_cross(a, b, polygon[i], polygon[(i + 1) % n]))
}

pub(crate) fn segments_properly_cross(a: Point, b: Point, c: Point, d: Point) -> bool {
    let side = |p: Point, q: Point, r: Point| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    const EPS: f64 = 1e-12;
    let (d1, d2, d3, d4) = (side(a, b, c), side(a, b, d), side(c, d, a), side(c, d, b));
    // Strict signs: shared endpoints and collinear overlaps are not crossings.
    ((d1 > EPS && d2 < -EPS) || (d1 < -EPS && d2 > EPS)) && ((d3 > EPS && d4 < -EPS) || (d3 < -EPS && d4 > EPS))
}
