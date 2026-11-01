//! Extending a line or arc on to the next curve it meets, splitting one in two, and cutting a
//! corner off straight.

use super::modify::{replace_end, Track};
use super::shapes::{crossing, gap, lerp, on_circle};
use super::*;
use simple3d_sketch::{sample_curve, ConstraintKind, Ellipse, Shape};
use std::f64::consts::TAU;

/// How finely curves are followed when looking for where an extension meets one.
const SEGMENTS: u32 = 96;
/// Fractions of a curve this close to its ends are its ends.
const AT_END: f64 = 1e-6;

/// The two lines meeting at `corner`, each with its far end, when exactly two lines and nothing
/// else meet there.
pub(crate) fn corner_lines(
    sketch: &Sketch,
    corner: PointId,
) -> Result<((CurveId, PointId), (CurveId, PointId)), &'static str> {
    let lines: Vec<(CurveId, PointId)> = sketch
        .curves
        .iter()
        .filter_map(|c| match c.shape {
            Shape::Line { a, b } if a == corner => Some((c.id, b)),
            Shape::Line { a, b } if b == corner => Some((c.id, a)),
            _ => None,
        })
        .collect();
    let &[(first, far1), (second, far2)] = &lines[..] else {
        return Err("Click a corner where exactly two lines meet");
    };
    if sketch.curves.iter().filter(|c| c.shape.points().contains(&corner)).count() > 2 {
        return Err("Click a corner where exactly two lines meet");
    }
    Ok(((first, far1), (second, far2)))
}

/// Cut the corner at `corner`, where two lines meet, off with a line from `distance` along one to
/// `distance` along the other, its length held by a dimension. The new line, or why not.
pub(crate) fn chamfer(sketch: &mut Sketch, corner: PointId, distance: f64) -> Result<CurveId, &'static str> {
    if distance <= 0.0 {
        return Err("Set a chamfer distance above zero");
    }
    let ((first, far1), (second, far2)) = corner_lines(sketch, corner)?;
    let p = sketch.at(corner);
    let back = |far: PointId| -> Result<[f64; 2], &'static str> {
        let q = sketch.at(far);
        let length = gap(p, q);
        if distance >= length - 1e-6 {
            return Err("The distance is too big for these lines");
        }
        Ok(lerp(p, q, distance / length))
    };
    let (t1, t2) = (back(far1)?, back(far2)?);
    if gap(t1, t2) < 1e-6 {
        return Err("The lines run straight on; there is no corner to cut");
    }
    let (a, b) = (sketch.add_point(t1), sketch.add_point(t2));
    replace_end(sketch, first, corner, a);
    replace_end(sketch, second, corner, b);
    let line = sketch.add_curve(Shape::Line { a, b }, false);
    sketch.push_constraint(ConstraintKind::Distance { a, b, value: gap(t1, t2) });
    Ok(line)
}

/// Cut curve `id` in two at the place on it nearest `at`, or at `point` when one is given that
/// lies on it. The two pieces stay in line (lines) or on one ellipse or circle (arcs, which share
/// their centre and the point between them).
pub(crate) fn split(
    sketch: &mut Sketch,
    id: CurveId,
    at: [f64; 2],
    point: Option<PointId>,
) -> Result<(), &'static str> {
    let curve = sketch.curve(id).ok_or("Click a line or an arc")?.clone();
    let place = match curve.shape {
        Shape::Line { .. } | Shape::Arc { .. } => {
            let track = Track::of(sketch, id).ok_or("Click a line or an arc")?;
            let t = track.place(at);
            if !(AT_END..=1.0 - AT_END).contains(&t) {
                return Err("Click away from its ends");
            }
            track.at(t)
        }
        Shape::EllipseArc { center, major, minor, .. } => {
            let e = Ellipse::new(sketch.at(center), sketch.at(major), minor);
            e.at(e.angle_of(at))
        }
        _ => return Err("Split works on lines and arcs; trim a circle or ellipse instead"),
    };
    let (_, last) = curve.shape.ends().expect("lines and arcs have ends");
    let middle = point.filter(|&p| gap(sketch.at(p), place) < 1e-6).unwrap_or_else(|| sketch.add_point(place));
    if curve.shape.ends().is_some_and(|(a, b)| a == middle || b == middle) {
        return Err("Click away from its ends");
    }
    sketch.regions.retain(|pick| pick.sides.iter().all(|&(curve, _)| curve != id));
    let tail = match curve.shape {
        Shape::Arc { center, end, .. } => Shape::Arc { center, start: middle, end },
        Shape::EllipseArc { center, major, minor, end, .. } => {
            Shape::EllipseArc { center, major, minor, start: middle, end }
        }
        _ => Shape::Line { a: middle, b: last },
    };
    // The tail goes in before the curve lets go of its end, which would otherwise be dropped unused.
    let piece = sketch.add_curve(tail, curve.construction);
    replace_end(sketch, id, last, middle);
    if curve.shape.is_line() {
        let square = sketch.constraints.iter().find_map(|c| match c.kind {
            ConstraintKind::Horizontal { line } if line == id => Some(ConstraintKind::Horizontal { line: piece }),
            ConstraintKind::Vertical { line } if line == id => Some(ConstraintKind::Vertical { line: piece }),
            _ => None,
        });
        sketch.push_constraint(square.unwrap_or(ConstraintKind::Parallel { a: id, b: piece }));
    }
    Ok(())
}

/// The path an end of curve `id` would run on along: on past a line's end, or on round an arc's
/// circle the rest of the way; with the end point it runs from.
fn onward(sketch: &Sketch, id: CurveId, at: [f64; 2]) -> Result<(PointId, Vec<[f64; 2]>), &'static str> {
    let curve = sketch.curve(id).ok_or("Click near the end of a line or an arc")?;
    let track = Track::of(sketch, id).filter(|_| curve.shape.ends().is_some());
    let track = track.ok_or("Extend works on lines and arcs")?;
    let (first, last) = curve.shape.ends().expect("checked above");
    let at_end = track.place(at) > 0.5;
    let end = if at_end { last } else { first };
    Ok(match track {
        Track::Line { a, b } => {
            let (from, to) = if at_end { (a, b) } else { (b, a) };
            let length = gap(from, to).max(1e-9);
            let reach = sketch.bounds().map_or(1e3, |(lo, hi)| 4.0 * gap(lo, hi) + 1e3);
            let d = [(to[0] - from[0]) / length, (to[1] - from[1]) / length];
            (end, vec![to, [to[0] + d[0] * reach, to[1] + d[1] * reach]])
        }
        Track::Round { centre, radius, from, sweep, .. } => {
            let rest = TAU - sweep;
            let (start, turn) = if at_end { (from + sweep, rest) } else { (from, -rest) };
            let steps = SEGMENTS as usize;
            (end, (0..=steps).map(|i| on_circle(centre, radius, start + turn * i as f64 / steps as f64)).collect())
        }
    })
}

/// Run the end of curve `id` nearest `at` on to the first curve it meets, and hold it there when
/// that curve can hold a point.
pub(crate) fn extend(sketch: &mut Sketch, id: CurveId, at: [f64; 2]) -> Result<(), &'static str> {
    let (end, path) = onward(sketch, id, at)?;
    if sketch.curves.iter().filter(|c| c.shape.points().contains(&end)).count() > 1 {
        return Err("That end joins another curve; nothing is free to extend");
    }
    if sketch.point(end).is_some_and(|p| p.fixed) {
        return Err("That end is fixed where it is");
    }
    let start = sketch.at(end);
    let mut hit: Option<([f64; 2], CurveId)> = None;
    'path: for leg in path.windows(2) {
        let mut best: Option<(f64, CurveId)> = None;
        for other in sketch.curves.iter().filter(|c| c.id != id) {
            let samples = sample_curve(sketch, other, SEGMENTS);
            let mut points = samples.points;
            if samples.closed && !points.is_empty() {
                points.push(points[0]);
            }
            for segment in points.windows(2) {
                let Some(t) = crossing(leg[0], leg[1], segment[0], segment[1]) else { continue };
                let there = lerp(leg[0], leg[1], t);
                if gap(there, start) > 1e-6 && best.is_none_or(|(most, _)| t < most) {
                    best = Some((t, other.id));
                }
            }
        }
        if let Some((t, other)) = best {
            hit = Some((lerp(leg[0], leg[1], t), other));
            break 'path;
        }
    }
    let (there, against) = hit.ok_or("Nothing lies ahead of that end to extend to")?;
    if let Some(point) = sketch.point_mut(end) {
        point.at = there;
    }
    let holdable =
        sketch.curve(against).is_some_and(|c| c.shape.is_line() || c.shape.is_round() || c.shape.is_elliptic());
    if holdable {
        sketch.push_constraint(ConstraintKind::OnCurve { point: end, curve: against });
    }
    Ok(())
}
