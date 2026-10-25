//! Tangency, which means something different for each pair of curves.

use super::equations::Eval;
use super::system::System;
use crate::curve::CurveId;
use crate::{cross, distance, length, sub};

/// The point two curves share as an end of both, if any.
fn shared_end(sys: &System, a: CurveId, b: CurveId) -> Option<u32> {
    let (a0, a1) = sys.sketch.curve(a)?.shape.ends()?;
    let (b0, b1) = sys.sketch.curve(b)?.shape.ends()?;
    [a0, a1].into_iter().find(|p| *p == b0 || *p == b1)
}

/// The equation keeping `a` and `b` tangent:
///
/// * two curves sharing an end point leave it in one direction, which is the only way a spline is
///   tangent to anything, and how a line runs smoothly into an arc it ends on;
/// * a line and a round curve otherwise touch: the centre is a radius from the line;
/// * two round curves touch, from outside or, when `inside`, from inside.
pub(crate) fn of(sys: &System, a: CurveId, b: CurveId, inside: bool) -> Result<Vec<(Vec<usize>, Eval)>, &'static str> {
    let shape = |c: CurveId| sys.sketch.curve(c).map(|c| c.shape.clone()).ok_or("missing");
    let (sa, sb) = (shape(a)?, shape(b)?);
    let deps: Vec<usize> = sys.curve_vars(a).into_iter().chain(sys.curve_vars(b)).collect();
    let x0 = &sys.start;
    if let Some(at) = shared_end(sys, a, b) {
        if sys.end_direction(x0, a, at).is_some() && sys.end_direction(x0, b, at).is_some() {
            let eval: Eval = Box::new(move |s, x| {
                let (d1, d2) = (s.end_direction(x, a, at).unwrap(), s.end_direction(x, b, at).unwrap());
                cross(d1, d2) / (length(d1) * length(d2)).max(1e-12)
            });
            return Ok(vec![(deps, eval)]);
        }
    }
    let (line, round) = match (sa.is_line(), sb.is_line(), sa.is_round(), sb.is_round()) {
        (true, _, _, true) => (Some(a), b),
        (_, true, true, _) => (Some(b), a),
        (_, _, true, true) => (None, b),
        _ => return Err("A spline can only be made tangent where it ends on the other curve"),
    };
    match line {
        Some(line) => {
            let (p, q) = sys.line(x0, line).unwrap();
            let (c, _) = sys.round(x0, round).unwrap();
            let sign = if cross(sub(q, p), sub(c, p)) < 0.0 { -1.0 } else { 1.0 };
            let eval: Eval = Box::new(move |s, x| {
                let (p, q) = s.line(x, line).unwrap();
                let (c, r) = s.round(x, round).unwrap();
                cross(sub(q, p), sub(c, p)) / distance(p, q).max(1e-12) * sign - r
            });
            Ok(vec![(deps, eval)])
        }
        None => {
            let eval: Eval = Box::new(move |s, x| {
                let (c1, r1) = s.round(x, a).unwrap();
                let (c2, r2) = s.round(x, b).unwrap();
                let gap = distance(c1, c2);
                if inside {
                    gap - (r1 - r2).abs()
                } else {
                    gap - (r1 + r2)
                }
            });
            Ok(vec![(deps, eval)])
        }
    }
}
