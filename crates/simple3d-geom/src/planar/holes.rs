//! Bridging a hole into its surrounding loop, so a region with holes can be
//! ear-clipped as one simple polygon.

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

/// Splice `hole` into `outer` along a bridge, turning a polygon with a hole into
/// a single (self-touching) loop that an ear clipper can eat.
///
/// The bridge runs from the hole's rightmost vertex to the nearest outer vertex
/// it can reach without crossing any edge of either loop. Both endpoints appear
/// twice in the result, which is what makes the seam infinitely thin and leaves
/// the enclosed area unchanged.
pub(crate) fn bridge_hole(
    outer: &mut Vec<u32>,
    outer_points: &mut Vec<Point>,
    hole: &[u32],
    hole_points: &[Point],
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
        !crosses_any(from, to, outer_points) && !crosses_any(from, to, hole_points)
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

/// Whether the open segment `a`-`b` properly crosses any edge of `polygon`.
/// Touching at an endpoint does not count: a bridge is *meant* to land on a
/// vertex of both loops.
pub(crate) fn crosses_any(a: Point, b: Point, polygon: &[Point]) -> bool {
    let n = polygon.len();
    (0..n).any(|i| segments_properly_cross(a, b, polygon[i], polygon[(i + 1) % n]))
}

pub(crate) fn segments_properly_cross(a: Point, b: Point, c: Point, d: Point) -> bool {
    let side = |p: Point, q: Point, r: Point| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    const EPS: f64 = 1e-12;
    let (d1, d2, d3, d4) = (side(a, b, c), side(a, b, d), side(c, d, a), side(c, d, b));
    // Strict signs on both segments: shared endpoints and collinear overlaps
    // fall through as "not crossing", which is what a bridge needs.
    ((d1 > EPS && d2 < -EPS) || (d1 < -EPS && d2 > EPS)) && ((d3 > EPS && d4 < -EPS) || (d3 < -EPS && d4 > EPS))
}
