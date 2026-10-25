//! The equations each kind of constraint stands for. Lengths are in millimetres and directions as
//! sines of angles, so a residual of a millionth means the same thing everywhere.

use super::system::System;
use crate::constraint::ConstraintKind;
use crate::curve::CurveId;
use crate::point::PointId;
use crate::{cross, distance, dot, length, sub};

pub(crate) type Eval = Box<dyn Fn(&System, &[f64]) -> f64>;

/// Never divide by a length that has collapsed.
fn safe(len: f64) -> f64 {
    len.max(1e-12)
}

/// Which side of zero `value` is as the solve begins, so a signed measurement keeps its side.
fn side(value: f64) -> f64 {
    if value < 0.0 {
        -1.0
    } else {
        1.0
    }
}

/// The angle from direction `a` to `b`, in (-pi, pi].
pub fn angle_between(a: [f64; 2], b: [f64; 2]) -> f64 {
    cross(a, b).atan2(dot(a, b))
}

/// The equations of `kind`, each with the unknowns it reads, or why the constraint cannot be put on
/// what it names.
pub(crate) fn of(sys: &System, kind: &ConstraintKind) -> Result<Vec<(Vec<usize>, Eval)>, &'static str> {
    use ConstraintKind::*;
    let x0 = &sys.start;
    let pv = |p: PointId| sys.point_vars(p);
    let cv = |c: CurveId| sys.curve_vars(c);
    let both = |a: Vec<usize>, b: Vec<usize>| a.into_iter().chain(b).collect::<Vec<usize>>();
    let line = |c: CurveId| sys.line(x0, c).ok_or("That constraint needs a line");
    let round = |c: CurveId| sys.round(x0, c).ok_or("That constraint needs an arc or a circle");
    let mut out: Vec<(Vec<usize>, Eval)> = Vec::new();
    match *kind {
        Coincident { a, b } => {
            for axis in 0..2 {
                out.push((both(pv(a), pv(b)), Box::new(move |s, x| s.pos(x, b)[axis] - s.pos(x, a)[axis])));
            }
        }
        Fix { point, at } => {
            for axis in 0..2 {
                out.push((pv(point), Box::new(move |s, x| s.pos(x, point)[axis] - at[axis])));
            }
        }
        Horizontal { line: l } | Vertical { line: l } => {
            line(l)?;
            let axis = if matches!(kind, Horizontal { .. }) { 1 } else { 0 };
            out.push((cv(l), Box::new(move |s, x| {
                let (p, q) = s.line(x, l).unwrap();
                q[axis] - p[axis]
            })));
        }
        Parallel { a, b } | Perpendicular { a, b } => {
            line(a)?;
            line(b)?;
            let parallel = matches!(kind, Parallel { .. });
            out.push((both(cv(a), cv(b)), Box::new(move |s, x| {
                let (p, q) = s.line(x, a).unwrap();
                let (r, t) = s.line(x, b).unwrap();
                let (d1, d2) = (sub(q, p), sub(t, r));
                let measure = if parallel { cross(d1, d2) } else { dot(d1, d2) };
                measure / safe(length(d1) * length(d2))
            })));
        }
        Angle { a, b, value } => {
            line(a)?;
            line(b)?;
            out.push((both(cv(a), cv(b)), Box::new(move |s, x| {
                let (p, q) = s.line(x, a).unwrap();
                let (r, t) = s.line(x, b).unwrap();
                let off = angle_between(sub(q, p), sub(t, r)) - value.to_radians();
                // Wrapped, so a full turn off reads as on.
                (off + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
            })));
        }
        Equal { a, b } => {
            let (ca, cb) = (sys.sketch.curve(a).ok_or("missing")?, sys.sketch.curve(b).ok_or("missing")?);
            if ca.shape.is_line() && cb.shape.is_line() {
                out.push((both(cv(a), cv(b)), Box::new(move |s, x| {
                    let (p, q) = s.line(x, a).unwrap();
                    let (r, t) = s.line(x, b).unwrap();
                    distance(p, q) - distance(r, t)
                })));
            } else if ca.shape.is_round() && cb.shape.is_round() {
                out.push((both(cv(a), cv(b)), Box::new(move |s, x| s.round(x, a).unwrap().1 - s.round(x, b).unwrap().1)));
            } else {
                return Err("Equal needs two lines or two round curves");
            }
        }
        Tangent { a, b, inside } => out.extend(super::tangent::of(sys, a, b, inside)?),
        Symmetric { a, b, line: l } => {
            line(l)?;
            let deps = both(both(pv(a), pv(b)), cv(l));
            out.push((deps.clone(), Box::new(move |s, x| {
                let (p, q) = s.line(x, l).unwrap();
                let mid = crate::scale(crate::add(s.pos(x, a), s.pos(x, b)), 0.5);
                cross(sub(q, p), sub(mid, p)) / safe(distance(p, q))
            })));
            out.push((deps, Box::new(move |s, x| {
                let (p, q) = s.line(x, l).unwrap();
                dot(sub(s.pos(x, b), s.pos(x, a)), sub(q, p)) / safe(distance(p, q))
            })));
        }
        Midpoint { point, line: l } => {
            line(l)?;
            for axis in 0..2 {
                out.push((both(pv(point), cv(l)), Box::new(move |s, x| {
                    let (p, q) = s.line(x, l).unwrap();
                    s.pos(x, point)[axis] - 0.5 * (p[axis] + q[axis])
                })));
            }
        }
        OnCurve { point, curve } => {
            let shape = &sys.sketch.curve(curve).ok_or("missing")?.shape;
            if shape.is_line() {
                out.push((both(pv(point), cv(curve)), Box::new(move |s, x| {
                    let (p, q) = s.line(x, curve).unwrap();
                    cross(sub(q, p), sub(s.pos(x, point), p)) / safe(distance(p, q))
                })));
            } else if shape.is_round() {
                out.push((both(pv(point), cv(curve)), Box::new(move |s, x| {
                    let (c, r) = s.round(x, curve).unwrap();
                    distance(s.pos(x, point), c) - r
                })));
            } else {
                return Err("A point can only be held on a line, an arc or a circle");
            }
        }
        Distance { a, b, value } => {
            out.push((both(pv(a), pv(b)), Box::new(move |s, x| distance(s.pos(x, a), s.pos(x, b)) - value)));
        }
        HorizontalDistance { a, b, value } | VerticalDistance { a, b, value } => {
            let axis = if matches!(kind, HorizontalDistance { .. }) { 0 } else { 1 };
            let sign = side(sys.pos(x0, b)[axis] - sys.pos(x0, a)[axis]);
            out.push((both(pv(a), pv(b)), Box::new(move |s, x| {
                (s.pos(x, b)[axis] - s.pos(x, a)[axis]) * sign - value
            })));
        }
        PointLineDistance { point, line: l, value } => {
            let (p, q) = line(l)?;
            let sign = side(cross(sub(q, p), sub(sys.pos(x0, point), p)));
            out.push((both(pv(point), cv(l)), Box::new(move |s, x| {
                let (p, q) = s.line(x, l).unwrap();
                cross(sub(q, p), sub(s.pos(x, point), p)) / safe(distance(p, q)) * sign - value
            })));
        }
        Radius { curve, value } => {
            round(curve)?;
            out.push((cv(curve), Box::new(move |s, x| s.round(x, curve).unwrap().1 - value)));
        }
    }
    Ok(out)
}
