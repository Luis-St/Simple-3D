//! The plane geometry the tools need: circles through three points, which way an arc runs, and the
//! corners of a regular polygon.

use std::f64::consts::TAU;

pub(crate) fn gap(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// The direction from `centre` to `p`, in radians.
pub(crate) fn angle_of(centre: [f64; 2], p: [f64; 2]) -> f64 {
    (p[1] - centre[1]).atan2(p[0] - centre[0])
}

/// How far counter-clockwise from angle `from` to angle `to`, in `[0, TAU)`.
pub(crate) fn ccw(from: f64, to: f64) -> f64 {
    (to - from).rem_euclid(TAU)
}

/// The point at `angle` on the circle about `centre` of `radius`.
pub(crate) fn on_circle(centre: [f64; 2], radius: f64, angle: f64) -> [f64; 2] {
    [centre[0] + radius * angle.cos(), centre[1] + radius * angle.sin()]
}

/// The centre and radius of the circle through `a`, `b` and `c`; `None` when they lie on one line.
pub(crate) fn circle_through(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> Option<([f64; 2], f64)> {
    let d = 2.0 * (a[0] * (b[1] - c[1]) + b[0] * (c[1] - a[1]) + c[0] * (a[1] - b[1]));
    let scale = gap(a, b).max(gap(b, c)).max(gap(a, c));
    if d.abs() < 1e-9 * scale * scale {
        return None;
    }
    let sq = |p: [f64; 2]| p[0] * p[0] + p[1] * p[1];
    let x = (sq(a) * (b[1] - c[1]) + sq(b) * (c[1] - a[1]) + sq(c) * (a[1] - b[1])) / d;
    let y = (sq(a) * (c[0] - b[0]) + sq(b) * (a[0] - c[0]) + sq(c) * (b[0] - a[0])) / d;
    let centre = [x, y];
    Some((centre, gap(centre, a)))
}

/// The arc from `start` to `end` bent through `through`, as its centre and its ends in the
/// counter-clockwise order an arc is stored in; `None` when the three lie on one line.
pub(crate) fn arc_through(start: [f64; 2], end: [f64; 2], through: [f64; 2]) -> Option<([f64; 2], [f64; 2], [f64; 2])> {
    let (centre, _) = circle_through(start, end, through)?;
    let (s, e, t) = (angle_of(centre, start), angle_of(centre, end), angle_of(centre, through));
    // Counter-clockwise from start to end must pass the bend, or the arc runs the other way round.
    Some(if ccw(s, t) <= ccw(s, e) { (centre, start, end) } else { (centre, end, start) })
}

/// The corners of a regular polygon of `sides` about `centre`, the first at `corner`.
pub(crate) fn polygon(centre: [f64; 2], corner: [f64; 2], sides: u32) -> Vec<[f64; 2]> {
    let radius = gap(centre, corner);
    let first = angle_of(centre, corner);
    (0..sides).map(|k| on_circle(centre, radius, first + TAU * k as f64 / sides as f64)).collect()
}

/// The four corners of the rectangle about `centre` with a corner at `corner`, counter-clockwise
/// from `corner` when it is up and to the right.
pub(crate) fn centred_rectangle(centre: [f64; 2], corner: [f64; 2]) -> [[f64; 2]; 4] {
    let (dx, dy) = (corner[0] - centre[0], corner[1] - centre[1]);
    let at = |sx: f64, sy: f64| [centre[0] + sx * dx, centre[1] + sy * dy];
    [at(1.0, 1.0), at(-1.0, 1.0), at(-1.0, -1.0), at(1.0, -1.0)]
}

/// Where segments `a`-`b` and `c`-`d` cross, as the fraction along `a`-`b`; `None` when they do not
/// or run parallel. Ends count, so a curve ending on the segment crosses it.
pub(crate) fn crossing(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> Option<f64> {
    let r = [b[0] - a[0], b[1] - a[1]];
    let s = [d[0] - c[0], d[1] - c[1]];
    let denom = r[0] * s[1] - r[1] * s[0];
    if denom.abs() < 1e-12 {
        return None;
    }
    let q = [c[0] - a[0], c[1] - a[1]];
    let t = (q[0] * s[1] - q[1] * s[0]) / denom;
    let u = (q[0] * r[1] - q[1] * r[0]) / denom;
    let slack = 1e-9;
    ((-slack..=1.0 + slack).contains(&t) && (-slack..=1.0 + slack).contains(&u)).then_some(t.clamp(0.0, 1.0))
}

/// The fraction along `a`-`b` of the point nearest `p`.
pub(crate) fn fraction_along(a: [f64; 2], b: [f64; 2], p: [f64; 2]) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let length = d[0] * d[0] + d[1] * d[1];
    if length < 1e-24 {
        return 0.0;
    }
    ((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / length
}

pub(crate) fn lerp(a: [f64; 2], b: [f64; 2], t: f64) -> [f64; 2] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_circle_through_three_points_has_them_all_on_it() {
        let (centre, radius) = circle_through([0.0, 0.0], [10.0, 0.0], [5.0, 5.0]).unwrap();
        assert!(gap(centre, [5.0, 0.0]) < 1e-9 && (radius - 5.0).abs() < 1e-9);
        assert!(circle_through([0.0, 0.0], [1.0, 1.0], [2.0, 2.0]).is_none(), "points on a line make no circle");
    }

    #[test]
    fn an_arc_through_a_point_below_its_ends_runs_round_underneath() {
        // Bent upward the arc runs counter-clockwise from the right end to the left.
        let (_, start, end) = arc_through([0.0, 0.0], [10.0, 0.0], [5.0, 3.0]).unwrap();
        assert_eq!((start, end), ([10.0, 0.0], [0.0, 0.0]));
        let (_, start, end) = arc_through([0.0, 0.0], [10.0, 0.0], [5.0, -3.0]).unwrap();
        assert_eq!((start, end), ([0.0, 0.0], [10.0, 0.0]));
    }
}
