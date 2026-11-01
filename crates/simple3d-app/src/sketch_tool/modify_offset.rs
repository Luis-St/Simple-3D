//! Offsetting: a copy of a curve and the lines and arcs joined end to end with it, a distance to
//! one side, the copies' corners where their neighbours meet again.

use super::shapes::gap;
use super::*;
use simple3d_sketch::{ConstraintKind, Shape};

/// A piece of the offset copy, in the direction the chain runs.
#[derive(Clone, Copy)]
enum Piece {
    Line {
        from: [f64; 2],
        to: [f64; 2],
    },
    /// About the original's centre point; `ccw` when the chain runs counter-clockwise round it.
    Round {
        centre: [f64; 2],
        radius: f64,
        ccw: bool,
        from: [f64; 2],
        to: [f64; 2],
    },
}

impl Piece {
    fn ends(&self) -> ([f64; 2], [f64; 2]) {
        match *self {
            Piece::Line { from, to } | Piece::Round { from, to, .. } => (from, to),
        }
    }
}

/// The line or arc curve `id` is, as its ends, when it is one.
fn ends_of(sketch: &Sketch, id: CurveId) -> Option<(PointId, PointId)> {
    let curve = sketch.curve(id)?;
    matches!(curve.shape, Shape::Line { .. } | Shape::Arc { .. }).then(|| curve.shape.ends()).flatten()
}

/// The lines and arcs joined end to end with `id`, where only two curves meet, each with whether
/// the chain runs along it from its start; and whether the chain closes on itself.
fn chain(sketch: &Sketch, id: CurveId) -> (Vec<(CurveId, bool)>, bool) {
    // The curve running on from `current` at its point `from`, with whether it starts there.
    let next = |current: CurveId, from: PointId| -> Option<(CurveId, bool)> {
        let using: Vec<CurveId> =
            sketch.curves.iter().filter(|c| c.shape.points().contains(&from)).map(|c| c.id).collect();
        if using.len() != 2 {
            return None;
        }
        let other = *using.iter().find(|&&c| c != current)?;
        let (a, b) = ends_of(sketch, other)?;
        (a == from || b == from).then_some((other, a == from))
    };
    // Where the chain leaves piece `(id, forward)`: its far end, or with `back` its near one.
    let tip = |(id, forward): (CurveId, bool), back: bool| {
        let (a, b) = ends_of(sketch, id).expect("chains hold lines and arcs");
        if forward != back {
            b
        } else {
            a
        }
    };
    let mut out = vec![(id, true)];
    loop {
        let last = *out.last().unwrap();
        let Some(step) = next(last.0, tip(last, false)) else { break };
        if step.0 == id {
            return (out, true);
        }
        if out.iter().any(|&(k, _)| k == step.0) {
            break;
        }
        out.push(step);
    }
    loop {
        let first = out[0];
        let Some((other, starts_here)) = next(first.0, tip(first, true)) else { break };
        if out.iter().any(|&(k, _)| k == other) {
            break;
        }
        // Run into the first piece's start: along the curve when it ends there.
        out.insert(0, (other, !starts_here));
    }
    (out, false)
}

/// Piece `(id, forward)` moved `off` to the left of the way the chain runs.
fn moved(sketch: &Sketch, (id, forward): (CurveId, bool), off: f64) -> Result<Piece, &'static str> {
    let at = |p: PointId| sketch.at(p);
    let (from, to) = ends_of(sketch, id).map(|(a, b)| if forward { (at(a), at(b)) } else { (at(b), at(a)) }).unwrap();
    Ok(match sketch.curve(id).map(|c| c.shape.clone()) {
        Some(Shape::Arc { center, .. }) => {
            let c = at(center);
            // Left of counter-clockwise travel is in toward the centre.
            let radius = gap(c, from) + if forward { -off } else { off };
            if radius <= 1e-6 {
                return Err("The offset is bigger than an arc's radius");
            }
            let out = |p: [f64; 2]| {
                let d = gap(c, p).max(1e-12);
                [c[0] + (p[0] - c[0]) / d * radius, c[1] + (p[1] - c[1]) / d * radius]
            };
            Piece::Round { centre: c, radius, ccw: forward, from: out(from), to: out(to) }
        }
        _ => {
            let length = gap(from, to).max(1e-12);
            let n = [-(to[1] - from[1]) / length * off, (to[0] - from[0]) / length * off];
            Piece::Line { from: [from[0] + n[0], from[1] + n[1]], to: [to[0] + n[0], to[1] + n[1]] }
        }
    })
}

/// Where pieces `a` and `b` meet again, nearest `near`: their crossing, or halfway between their
/// ends when they no longer cross.
fn corner(a: &Piece, b: &Piece, near: [f64; 2]) -> [f64; 2] {
    let (end, start) = (a.ends().1, b.ends().0);
    if gap(end, start) < 1e-9 {
        return end;
    }
    let circle = |p: &Piece| match *p {
        Piece::Round { centre, radius, .. } => Some((centre, radius)),
        Piece::Line { .. } => None,
    };
    let hits: Vec<[f64; 2]> = match (circle(a), circle(b)) {
        (None, None) => line_line(a.ends(), b.ends()).into_iter().collect(),
        (Some(c), None) => line_circle(b.ends(), c),
        (None, Some(c)) => line_circle(a.ends(), c),
        (Some(c1), Some(c2)) => circle_circle(c1, c2),
    };
    hits.into_iter()
        .min_by(|p, q| gap(*p, near).total_cmp(&gap(*q, near)))
        .unwrap_or([(end[0] + start[0]) / 2.0, (end[1] + start[1]) / 2.0])
}

fn line_line((p, q): ([f64; 2], [f64; 2]), (r, s): ([f64; 2], [f64; 2])) -> Option<[f64; 2]> {
    let (d1, d2) = ([q[0] - p[0], q[1] - p[1]], [s[0] - r[0], s[1] - r[1]]);
    let denom = d1[0] * d2[1] - d1[1] * d2[0];
    (denom.abs() > 1e-12).then(|| {
        let t = ((r[0] - p[0]) * d2[1] - (r[1] - p[1]) * d2[0]) / denom;
        [p[0] + d1[0] * t, p[1] + d1[1] * t]
    })
}

fn line_circle((p, q): ([f64; 2], [f64; 2]), (c, r): ([f64; 2], f64)) -> Vec<[f64; 2]> {
    let d = [q[0] - p[0], q[1] - p[1]];
    let f = [p[0] - c[0], p[1] - c[1]];
    let (a, b, k) = (d[0] * d[0] + d[1] * d[1], 2.0 * (f[0] * d[0] + f[1] * d[1]), f[0] * f[0] + f[1] * f[1] - r * r);
    let disc = b * b - 4.0 * a * k;
    if a < 1e-24 || disc < 0.0 {
        return Vec::new();
    }
    [-1.0, 1.0].iter().map(|s| (-b + s * disc.sqrt()) / (2.0 * a)).map(|t| [p[0] + d[0] * t, p[1] + d[1] * t]).collect()
}

fn circle_circle((c1, r1): ([f64; 2], f64), (c2, r2): ([f64; 2], f64)) -> Vec<[f64; 2]> {
    let d = gap(c1, c2);
    if d < 1e-12 || d > r1 + r2 || d < (r1 - r2).abs() {
        return Vec::new();
    }
    let a = (r1 * r1 - r2 * r2 + d * d) / (2.0 * d);
    let h = (r1 * r1 - a * a).max(0.0).sqrt();
    let u = [(c2[0] - c1[0]) / d, (c2[1] - c1[1]) / d];
    let m = [c1[0] + u[0] * a, c1[1] + u[1] * a];
    vec![[m[0] - u[1] * h, m[1] + u[0] * h], [m[0] + u[1] * h, m[1] - u[0] * h]]
}

/// Copy curve `id`, with the lines and arcs joined to it, `distance` to the side of it `at` lies
/// on. Copied lines are held parallel to theirs, copied arcs and circles keep their centre point.
/// How many curves were added, or why not.
pub(crate) fn offset(sketch: &mut Sketch, id: CurveId, at: [f64; 2], distance: f64) -> Result<usize, &'static str> {
    if distance <= 0.0 {
        return Err("Set an offset distance above zero");
    }
    let curve = sketch.curve(id).ok_or("Click a line, an arc or a circle")?.clone();
    if let Shape::Circle { center, radius } = curve.shape {
        let inside = gap(at, sketch.at(center)) < radius;
        let radius = if inside { radius - distance } else { radius + distance };
        if radius <= 1e-6 {
            return Err("The offset is bigger than the circle's radius");
        }
        sketch.add_curve(Shape::Circle { center, radius }, curve.construction);
        return Ok(1);
    }
    let (from, to) = ends_of(sketch, id).ok_or("Offset works on lines, arcs and circles")?;
    let (pieces, closed) = chain(sketch, id);
    let left = match curve.shape {
        Shape::Arc { center, .. } => gap(at, sketch.at(center)) < gap(sketch.at(from), sketch.at(center)),
        _ => {
            let (p, q) = (sketch.at(from), sketch.at(to));
            (q[0] - p[0]) * (at[1] - p[1]) - (q[1] - p[1]) * (at[0] - p[0]) > 0.0
        }
    };
    let off = if left { distance } else { -distance };
    let moved: Vec<Piece> = pieces.iter().map(|&piece| moved(sketch, piece, off)).collect::<Result<_, _>>()?;
    let n = moved.len();
    let joint_at = |i: usize| {
        let (id, forward) = pieces[i];
        let (a, b) = ends_of(sketch, id).unwrap();
        sketch.at(if forward { b } else { a })
    };
    // Corner `i` is where piece `i` ends and the next starts; an open chain adds its two free ends.
    let mut corners: Vec<[f64; 2]> =
        (0..n - usize::from(!closed)).map(|i| corner(&moved[i], &moved[(i + 1) % n], joint_at(i))).collect();
    if !closed {
        corners.insert(0, moved[0].ends().0);
        corners.push(moved[n - 1].ends().1);
    }
    let ids: Vec<PointId> = corners.iter().map(|&p| sketch.add_point(p)).collect();
    // A joint where the pieces run on smoothly stays smooth: the copies are held tangent there, and
    // a line touching such a joint takes its direction from that rather than from a parallel, which
    // round a closed chain would only repeat what the tangencies say.
    let smooth: Vec<bool> =
        (0..n).map(|i| (closed || i + 1 < n) && gap(moved[i].ends().1, moved[(i + 1) % n].ends().0) < 1e-9).collect();
    let mut copies = Vec::with_capacity(n);
    for (i, piece) in moved.iter().enumerate() {
        let (start, end) = if closed { (ids[(i + n - 1) % n], ids[i]) } else { (ids[i], ids[i + 1]) };
        let original = pieces[i].0;
        let construction = sketch.curve(original).is_some_and(|c| c.construction);
        let before = smooth[(i + n - 1) % n] && (closed || i > 0);
        copies.push(match *piece {
            Piece::Line { .. } => {
                let line = sketch.add_curve(Shape::Line { a: start, b: end }, construction);
                if !before && !smooth[i] {
                    sketch.push_constraint(ConstraintKind::Parallel { a: original, b: line });
                }
                line
            }
            Piece::Round { ccw, .. } => {
                let Some(Shape::Arc { center, .. }) = sketch.curve(original).map(|c| c.shape.clone()) else {
                    return Err("Offset works on lines, arcs and circles");
                };
                let (start, end) = if ccw { (start, end) } else { (end, start) };
                sketch.add_curve(Shape::Arc { center, start, end }, construction)
            }
        });
    }
    for i in (0..n).filter(|&i| smooth[i]) {
        sketch.push_constraint(ConstraintKind::Tangent { a: copies[i], b: copies[(i + 1) % n], inside: false });
    }
    Ok(n)
}
