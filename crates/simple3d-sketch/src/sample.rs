//! Curves as polylines: what is drawn on screen, intersected for regions and swept into solids.

use crate::curve::{Curve, Shape};
use crate::sketch::Sketch;
use std::f64::consts::TAU;

/// A curve's polyline, from its start to its end; a closed one does not repeat its first point.
#[derive(Clone, Debug, PartialEq)]
pub struct Samples {
    pub points: Vec<[f64; 2]>,
    pub closed: bool,
}

/// How many straight pieces a spline gets between two of its points, at `segments` per full turn.
fn per_span(segments: u32) -> usize {
    (segments as usize / 4).clamp(6, 48)
}

/// Curve `curve` of `sketch` as a polyline, with `segments` pieces to a full circle. Arcs, open
/// splines and lines start and end exactly on their end points, so curves sharing a point meet.
pub fn sample_curve(sketch: &Sketch, curve: &Curve, segments: u32) -> Samples {
    let segments = segments.max(8);
    let at = |id| sketch.at(id);
    match &curve.shape {
        Shape::Line { a, b } => Samples { points: vec![at(*a), at(*b)], closed: false },
        Shape::Circle { center, radius } => {
            let c = at(*center);
            let r = radius.abs();
            let points = (0..segments)
                .map(|i| {
                    let t = TAU * i as f64 / segments as f64;
                    [c[0] + r * t.cos(), c[1] + r * t.sin()]
                })
                .collect();
            Samples { points, closed: true }
        }
        Shape::Arc { center, start, end } => {
            let (c, s, e) = (at(*center), at(*start), at(*end));
            let r = crate::distance(s, c);
            let (a0, sweep) = arc_angles(c, s, e);
            let n = ((segments as f64 * sweep / TAU).ceil() as usize).max(2);
            let mut points: Vec<[f64; 2]> = (0..n)
                .map(|i| {
                    let t = a0 + sweep * i as f64 / n as f64;
                    [c[0] + r * t.cos(), c[1] + r * t.sin()]
                })
                .collect();
            points[0] = s;
            points.push(e);
            Samples { points, closed: false }
        }
        Shape::Spline { points, closed } => {
            let p: Vec<[f64; 2]> = points.iter().map(|&id| at(id)).collect();
            Samples { points: catmull_rom(&p, *closed, per_span(segments)), closed: *closed }
        }
        Shape::Bspline { points, closed } => {
            let p: Vec<[f64; 2]> = points.iter().map(|&id| at(id)).collect();
            let points = match closed {
                true => periodic_bspline(&p, per_span(segments)),
                false => clamped_bspline(&p, per_span(segments)),
            };
            Samples { points, closed: *closed }
        }
    }
}

/// An arc's start angle and its counter-clockwise sweep, in (0, 2 pi]; an arc whose ends meet is a
/// full turn.
pub(crate) fn arc_angles(c: [f64; 2], s: [f64; 2], e: [f64; 2]) -> (f64, f64) {
    let a0 = (s[1] - c[1]).atan2(s[0] - c[0]);
    let a1 = (e[1] - c[1]).atan2(e[0] - c[0]);
    let mut sweep = (a1 - a0).rem_euclid(TAU);
    if sweep < 1e-9 {
        sweep = TAU;
    }
    (a0, sweep)
}

/// A centripetal Catmull-Rom spline through `p`, `steps` pieces per span. An open one is extended
/// past its ends by mirroring, so it leaves its first point heading for the second.
fn catmull_rom(p: &[[f64; 2]], closed: bool, steps: usize) -> Vec<[f64; 2]> {
    let n = p.len();
    if n < 2 {
        return p.to_vec();
    }
    let get = |i: isize| -> [f64; 2] {
        if closed {
            p[i.rem_euclid(n as isize) as usize]
        } else if i < 0 {
            crate::sub(crate::scale(p[0], 2.0), p[1])
        } else if i as usize >= n {
            crate::sub(crate::scale(p[n - 1], 2.0), p[n - 2])
        } else {
            p[i as usize]
        }
    };
    let spans = if closed { n } else { n - 1 };
    let mut out = Vec::with_capacity(spans * steps + 1);
    for i in 0..spans as isize {
        let (p0, p1, p2, p3) = (get(i - 1), get(i), get(i + 1), get(i + 2));
        // Knot gaps by the square root of the chord, never zero so repeated points do not divide by it.
        let gap = |a: [f64; 2], b: [f64; 2]| crate::distance(a, b).sqrt().max(1e-9);
        let t1 = gap(p0, p1);
        let t2 = t1 + gap(p1, p2);
        let t3 = t2 + gap(p2, p3);
        for k in 0..steps {
            let t = t1 + (t2 - t1) * k as f64 / steps as f64;
            let lerp = |a: [f64; 2], b: [f64; 2], ta: f64, tb: f64| {
                let w = (t - ta) / (tb - ta);
                crate::add(crate::scale(a, 1.0 - w), crate::scale(b, w))
            };
            let a1 = lerp(p0, p1, 0.0, t1);
            let a2 = lerp(p1, p2, t1, t2);
            let a3 = lerp(p2, p3, t2, t3);
            let b1 = lerp(a1, a2, 0.0, t2);
            let b2 = lerp(a2, a3, t1, t3);
            out.push(if k == 0 { p1 } else { lerp(b1, b2, t1, t2) });
        }
    }
    if !closed {
        out.push(p[n - 1]);
    }
    out
}

/// An open cubic B-spline (lower degree for fewer points) clamped to start on the first control
/// point and end on the last, `steps` pieces per span.
fn clamped_bspline(p: &[[f64; 2]], steps: usize) -> Vec<[f64; 2]> {
    let n = p.len();
    if n < 2 {
        return p.to_vec();
    }
    let degree = 3.min(n - 1);
    let spans = n - degree;
    let mut knots = vec![0.0; degree + 1];
    knots.extend((1..spans).map(|i| i as f64));
    knots.extend(std::iter::repeat_n(spans as f64, degree + 1));
    let total = spans * steps;
    let mut out: Vec<[f64; 2]> = (0..total).map(|i| de_boor(p, &knots, degree, i as f64 / steps as f64)).collect();
    out[0] = p[0];
    out.push(p[n - 1]);
    out
}

/// The point at `t` of a B-spline with control points `p`, by de Boor's algorithm.
fn de_boor(p: &[[f64; 2]], knots: &[f64], degree: usize, t: f64) -> [f64; 2] {
    let n = p.len();
    let mut span = degree;
    while span < n - 1 && t >= knots[span + 1] {
        span += 1;
    }
    let mut d: Vec<[f64; 2]> = (0..=degree).map(|j| p[j + span - degree]).collect();
    for r in 1..=degree {
        for j in (r..=degree).rev() {
            let i = j + span - degree;
            let denom = knots[i + 1 + degree - r] - knots[i];
            let alpha = if denom.abs() < 1e-12 { 0.0 } else { (t - knots[i]) / denom };
            d[j] = crate::add(crate::scale(d[j - 1], 1.0 - alpha), crate::scale(d[j], alpha));
        }
    }
    d[degree]
}

/// A closed uniform cubic B-spline running round every control point, `steps` pieces per span.
fn periodic_bspline(p: &[[f64; 2]], steps: usize) -> Vec<[f64; 2]> {
    let n = p.len();
    let mut out = Vec::with_capacity(n * steps);
    for i in 0..n {
        let q = |k: usize| p[(i + k) % n];
        for s in 0..steps {
            let t = s as f64 / steps as f64;
            let (t2, t3) = (t * t, t * t * t);
            let w = [
                (1.0 - t).powi(3) / 6.0,
                (3.0 * t3 - 6.0 * t2 + 4.0) / 6.0,
                (-3.0 * t3 + 3.0 * t2 + 3.0 * t + 1.0) / 6.0,
                t3 / 6.0,
            ];
            let mut at = [0.0; 2];
            for (k, wk) in w.iter().enumerate() {
                at = crate::add(at, crate::scale(q(k), *wk));
            }
            out.push(at);
        }
    }
    out
}
