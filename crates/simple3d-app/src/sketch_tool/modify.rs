//! The modifying tools: trimming a curve back to the curves crossing it, and rounding a corner
//! where two lines meet.

use super::shapes::{angle_of, ccw, crossing, fraction_along, gap, lerp, on_circle};
use super::*;
use simple3d_sketch::{sample_curve, ConstraintKind, Shape};
use std::f64::consts::{PI, TAU};

/// How finely the other curves are followed when looking for where they cross.
const SEGMENTS: u32 = 96;
/// Crossings this close to a curve's end, as a fraction of it, are its end: where it joins the next.
const AT_END: f64 = 1e-6;

/// A line, arc or circle as trimming sees it: a run from 0 to 1.
#[derive(Clone, Copy)]
enum Track {
    Line {
        a: [f64; 2],
        b: [f64; 2],
    },
    /// From angle `from`, `sweep` counter-clockwise; all the way round for a circle.
    Round {
        centre: [f64; 2],
        radius: f64,
        from: f64,
        sweep: f64,
        closed: bool,
    },
}

impl Track {
    fn of(sketch: &Sketch, id: CurveId) -> Option<Track> {
        let at = |p: PointId| sketch.at(p);
        Some(match sketch.curve(id)?.shape {
            Shape::Line { a, b } => Track::Line { a: at(a), b: at(b) },
            Shape::Arc { center, start, end } => {
                let (c, s, e) = (at(center), at(start), at(end));
                let from = angle_of(c, s);
                let sweep = ccw(from, angle_of(c, e));
                let sweep = if sweep <= 0.0 { TAU } else { sweep };
                Track::Round { centre: c, radius: gap(c, s), from, sweep, closed: false }
            }
            Shape::Circle { center, radius } => {
                Track::Round { centre: at(center), radius, from: 0.0, sweep: TAU, closed: true }
            }
            _ => return None,
        })
    }

    /// How far along the run `p` lies, or the nearest place on it.
    fn place(&self, p: [f64; 2]) -> f64 {
        match *self {
            Track::Line { a, b } => fraction_along(a, b, p),
            Track::Round { centre, from, sweep, .. } => ccw(from, angle_of(centre, p)) / sweep,
        }
    }

    fn at(&self, t: f64) -> [f64; 2] {
        match *self {
            Track::Line { a, b } => lerp(a, b, t),
            Track::Round { centre, radius, from, sweep, .. } => on_circle(centre, radius, from + sweep * t),
        }
    }
}

/// Where the other curves cross curve `id`, as places along it with the curve crossing there, in order.
fn crossings(sketch: &Sketch, id: CurveId, track: &Track) -> Vec<(f64, CurveId)> {
    let polyline = |curve: &simple3d_sketch::Curve| {
        let samples = sample_curve(sketch, curve, SEGMENTS);
        let mut points = samples.points;
        if samples.closed && !points.is_empty() {
            points.push(points[0]);
        }
        points
    };
    let Some(own) = sketch.curve(id).map(polyline) else { return Vec::new() };
    let mut out: Vec<(f64, CurveId)> = Vec::new();
    for other in sketch.curves.iter().filter(|c| c.id != id) {
        let theirs = polyline(other);
        for mine in own.windows(2) {
            for segment in theirs.windows(2) {
                if let Some(t) = crossing(mine[0], mine[1], segment[0], segment[1]) {
                    out.push((track.place(lerp(mine[0], mine[1], t)), other.id));
                }
            }
        }
    }
    let closed = matches!(track, Track::Round { closed: true, .. });
    out.retain(|&(t, _)| closed || (AT_END..=1.0 - AT_END).contains(&t));
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.dedup_by(|a, b| (a.0 - b.0).abs() < AT_END);
    out
}

/// The point a trimmed end goes to: a sketch point already there, else a new one held on the curve
/// it was cut against when that can be held (a line or a round curve).
fn end_at(sketch: &mut Sketch, at: [f64; 2], against: CurveId) -> PointId {
    if let Some(point) = sketch.points.iter().find(|p| gap(p.at, at) < 1e-6) {
        return point.id;
    }
    let point = sketch.add_point(at);
    let holdable = sketch.curve(against).is_some_and(|c| c.shape.is_line() || c.shape.is_round());
    if holdable {
        sketch.push_constraint(ConstraintKind::OnCurve { point, curve: against });
    }
    point
}

/// Cut away the piece of curve `id` around `at` that runs between the curves crossing it; a curve
/// nothing crosses goes altogether. Why not, for a curve that cannot be trimmed.
pub(crate) fn trim(sketch: &mut Sketch, id: CurveId, at: [f64; 2]) -> Result<(), &'static str> {
    let track = Track::of(sketch, id).ok_or("Trim works on lines, arcs and circles")?;
    let cuts = crossings(sketch, id, &track);
    let here = track.place(at);
    let Some(shape) = sketch.curve(id).map(|c| c.shape.clone()) else { return Ok(()) };
    sketch.regions.retain(|pick| pick.sides.iter().all(|&(curve, _)| curve != id));
    if let Shape::Circle { center, .. } = shape {
        if cuts.len() < 2 {
            sketch.remove_curve(id);
            return Ok(());
        }
        // What is left runs on from the next cut after the pointer round to the one before it.
        let after = cuts.iter().find(|c| c.0 > here).unwrap_or(&cuts[0]);
        let before = cuts.iter().rev().find(|c| c.0 < here).unwrap_or(&cuts[cuts.len() - 1]);
        let start = end_at(sketch, track.at(after.0), after.1);
        let end = end_at(sketch, track.at(before.0), before.1);
        if let Some(curve) = sketch.curve_mut(id) {
            curve.shape = Shape::Arc { center, start, end };
        }
        return Ok(());
    }
    let low = cuts.iter().rev().find(|c| c.0 < here).copied();
    let high = cuts.iter().find(|c| c.0 > here).copied();
    let (first, last) = shape.ends().expect("lines and arcs have ends");
    match (low, high) {
        (None, None) => sketch.remove_curve(id),
        (None, Some((t, against))) => {
            let point = end_at(sketch, track.at(t), against);
            replace_end(sketch, id, first, point);
        }
        (Some((t, against)), None) => {
            let point = end_at(sketch, track.at(t), against);
            replace_end(sketch, id, last, point);
        }
        (Some((t0, low)), Some((t1, high))) => {
            let (cut0, cut1) = (end_at(sketch, track.at(t0), low), end_at(sketch, track.at(t1), high));
            let tail = match shape {
                Shape::Arc { center, end, .. } => Shape::Arc { center, start: cut1, end },
                _ => Shape::Line { a: cut1, b: last },
            };
            let construction = sketch.curve(id).is_some_and(|c| c.construction);
            replace_end(sketch, id, last, cut0);
            let piece = sketch.add_curve(tail, construction);
            keep_like(sketch, id, piece, cut1);
        }
    }
    Ok(())
}

/// Curve `id` ending on `with` where it ended on `old`, and `old` gone if nothing else uses it.
fn replace_end(sketch: &mut Sketch, id: CurveId, old: PointId, with: PointId) {
    if let Some(curve) = sketch.curve_mut(id) {
        match &mut curve.shape {
            Shape::Line { a, b } => {
                if *a == old {
                    *a = with;
                } else if *b == old {
                    *b = with;
                }
            }
            Shape::Arc { start, end, .. } => {
                if *start == old {
                    *start = with;
                } else if *end == old {
                    *end = with;
                }
            }
            _ => {}
        }
    }
    if !sketch.point_used(old) {
        sketch.remove_point(old);
    }
}

/// The piece split off curve `id`, starting at `start`, stays in line with it (a line) or on its
/// circle (an arc, which shares its centre).
fn keep_like(sketch: &mut Sketch, id: CurveId, piece: CurveId, start: PointId) {
    let held: Vec<ConstraintKind> = sketch
        .constraints
        .iter()
        .filter_map(|c| match c.kind {
            ConstraintKind::Horizontal { line } if line == id => Some(ConstraintKind::Horizontal { line: piece }),
            ConstraintKind::Vertical { line } if line == id => Some(ConstraintKind::Vertical { line: piece }),
            _ => None,
        })
        .collect();
    let square = !held.is_empty();
    for kind in held {
        sketch.push_constraint(kind);
    }
    if sketch.curve(id).is_some_and(|c| c.shape.is_line()) {
        // A copied horizontal or vertical already keeps the direction.
        if !square {
            sketch.push_constraint(ConstraintKind::Parallel { a: id, b: piece });
        }
        sketch.push_constraint(ConstraintKind::OnCurve { point: start, curve: id });
    } else {
        sketch.push_constraint(ConstraintKind::Equal { a: id, b: piece });
    }
}

/// Round the corner at `corner`, where two lines meet, with an arc of `radius` tangent to both and
/// held at that radius. The new arc, or why not.
pub(crate) fn fillet(sketch: &mut Sketch, corner: PointId, radius: f64) -> Result<CurveId, &'static str> {
    if radius <= 0.0 {
        return Err("Set a fillet radius above zero");
    }
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
    let p = sketch.at(corner);
    let away = |far: PointId| {
        let q = sketch.at(far);
        let length = gap(p, q);
        ([(q[0] - p[0]) / length, (q[1] - p[1]) / length], length)
    };
    let ((d1, len1), (d2, len2)) = (away(far1), away(far2));
    let angle = (d1[0] * d2[0] + d1[1] * d2[1]).clamp(-1.0, 1.0).acos();
    if !(1e-3..=PI - 1e-3).contains(&angle) {
        return Err("The lines run straight on; there is no corner to round");
    }
    let back = radius / (angle / 2.0).tan();
    if back >= len1.min(len2) - 1e-6 {
        return Err("The radius is too big for these lines");
    }
    let (t1, t2) = ([p[0] + d1[0] * back, p[1] + d1[1] * back], [p[0] + d2[0] * back, p[1] + d2[1] * back]);
    let bisector = [d1[0] + d2[0], d1[1] + d2[1]];
    let reach = radius / (angle / 2.0).sin() / bisector[0].hypot(bisector[1]);
    let centre = [p[0] + bisector[0] * reach, p[1] + bisector[1] * reach];
    let (a1, a2) = (sketch.add_point(t1), sketch.add_point(t2));
    let center = sketch.add_point(centre);
    replace_end(sketch, first, corner, a1);
    replace_end(sketch, second, corner, a2);
    // Counter-clockwise the short way round.
    let (start, end) = if ccw(angle_of(centre, t1), angle_of(centre, t2)) < PI { (a1, a2) } else { (a2, a1) };
    let arc = sketch.add_curve(Shape::Arc { center, start, end }, false);
    sketch.push_constraint(ConstraintKind::Tangent { a: first, b: arc, inside: false });
    sketch.push_constraint(ConstraintKind::Tangent { a: second, b: arc, inside: false });
    sketch.push_constraint(ConstraintKind::Radius { curve: arc, value: radius });
    Ok(arc)
}
