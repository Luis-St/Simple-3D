//! The tools that work on the selection: mirroring it across a line, copying it in a row or round a
//! centre, and moving, turning or scaling it. Each takes the curves picked and the points they are
//! drawn from, along with points picked on their own.

use super::settings::Settings;
use super::shapes::{angle_of, gap};
use super::*;
use simple3d_sketch::{ConstraintKind, Shape};
use std::collections::HashMap;

/// The selected curves, and every point the tool moves: theirs and the points picked on their own.
pub(crate) fn selected(tool: &SketchTool) -> (Vec<CurveId>, Vec<PointId>) {
    let curves: Vec<CurveId> =
        tool.selection.iter().filter_map(|p| if let Pick::Curve(id) = p { Some(*id) } else { None }).collect();
    let mut points: Vec<PointId> =
        curves.iter().filter_map(|&c| tool.sketch.curve(c)).flat_map(|c| c.shape.points()).collect();
    points.extend(tool.selection.iter().filter_map(|p| if let Pick::Point(id) = p { Some(*id) } else { None }));
    points.sort_unstable();
    points.dedup();
    (curves, points)
}

/// Shape `shape` drawn from the points `points` maps its own to; a mirror image (`flipped`) runs the
/// other way round, so an arc's ends swap.
fn remapped(shape: &Shape, points: &HashMap<PointId, PointId>, flipped: bool) -> Shape {
    let p = |id: &PointId| *points.get(id).unwrap_or(id);
    match shape {
        Shape::Line { a, b } => Shape::Line { a: p(a), b: p(b) },
        Shape::Arc { center, start, end } => match flipped {
            false => Shape::Arc { center: p(center), start: p(start), end: p(end) },
            true => Shape::Arc { center: p(center), start: p(end), end: p(start) },
        },
        Shape::Circle { center, radius } => Shape::Circle { center: p(center), radius: *radius },
        Shape::Ellipse { center, major, minor } => Shape::Ellipse { center: p(center), major: p(major), minor: *minor },
        Shape::EllipseArc { center, major, minor, start, end } => {
            let (start, end) = if flipped { (p(end), p(start)) } else { (p(start), p(end)) };
            Shape::EllipseArc { center: p(center), major: p(major), minor: *minor, start, end }
        }
        Shape::Spline { points: list, closed } => {
            Shape::Spline { points: list.iter().map(p).collect(), closed: *closed }
        }
        Shape::Bspline { points: list, closed } => {
            Shape::Bspline { points: list.iter().map(p).collect(), closed: *closed }
        }
    }
}

/// Constraint `kind` on the copies, its points and curves swapped through the maps and a fix moved
/// by `place`.
fn remap_constraint(
    kind: &ConstraintKind,
    p: impl Fn(PointId) -> PointId,
    c: impl Fn(CurveId) -> CurveId,
    place: impl Fn([f64; 2]) -> [f64; 2],
) -> ConstraintKind {
    use ConstraintKind::*;
    match *kind {
        Coincident { a, b } => Coincident { a: p(a), b: p(b) },
        Horizontal { line } => Horizontal { line: c(line) },
        Vertical { line } => Vertical { line: c(line) },
        Parallel { a, b } => Parallel { a: c(a), b: c(b) },
        Perpendicular { a, b } => Perpendicular { a: c(a), b: c(b) },
        Tangent { a, b, inside } => Tangent { a: c(a), b: c(b), inside },
        Equal { a, b } => Equal { a: c(a), b: c(b) },
        Symmetric { a, b, line } => Symmetric { a: p(a), b: p(b), line: c(line) },
        Midpoint { point, line } => Midpoint { point: p(point), line: c(line) },
        OnCurve { point, curve } => OnCurve { point: p(point), curve: c(curve) },
        Fix { point, at } => Fix { point: p(point), at: place(at) },
        Distance { a, b, value } => Distance { a: p(a), b: p(b), value },
        HorizontalDistance { a, b, value } => HorizontalDistance { a: p(a), b: p(b), value },
        VerticalDistance { a, b, value } => VerticalDistance { a: p(a), b: p(b), value },
        PointLineDistance { point, line, value } => PointLineDistance { point: p(point), line: c(line), value },
        Radius { curve, value } => Radius { curve: c(curve), value },
        Diameter { curve, value } => Diameter { curve: c(curve), value },
        Angle { a, b, value } => Angle { a: c(a), b: c(b), value },
    }
}

/// Whether a constraint only holds when the drawing keeps its turn to the sketch's axes.
fn square_to_axes(kind: &ConstraintKind) -> bool {
    use ConstraintKind::*;
    matches!(kind, Horizontal { .. } | Vertical { .. } | HorizontalDistance { .. } | VerticalDistance { .. })
}

/// Copy `curves` with their points moved by `place`, and the constraints among them along with them
/// unless `turned` (then those square to the axes stay behind). The copies' curve ids.
fn copy(sketch: &mut Sketch, curves: &[CurveId], place: &dyn Fn([f64; 2]) -> [f64; 2], turned: bool) -> Vec<CurveId> {
    let own: Vec<PointId> = curves.iter().filter_map(|&c| sketch.curve(c)).flat_map(|c| c.shape.points()).collect();
    let mut points = HashMap::new();
    for &id in &own {
        if !points.contains_key(&id) {
            let at = place(sketch.at(id));
            points.insert(id, sketch.add_point(at));
        }
    }
    let mut copies = HashMap::new();
    for &id in curves {
        let Some(curve) = sketch.curve(id).cloned() else { continue };
        copies.insert(id, sketch.add_curve(remapped(&curve.shape, &points, false), curve.construction));
    }
    let inside: Vec<ConstraintKind> = sketch
        .constraints
        .iter()
        .map(|c| &c.kind)
        .filter(|k| {
            k.points().iter().all(|p| points.contains_key(p)) && k.curves().iter().all(|c| copies.contains_key(c))
        })
        .filter(|k| !(turned && square_to_axes(k)))
        .map(|k| remap_constraint(k, |p| points[&p], |c| copies[&c], place))
        .collect();
    for kind in inside {
        sketch.push_constraint(kind);
    }
    curves.iter().filter_map(|c| copies.get(c).copied()).collect()
}

/// Mirror `curves` across line `line`: each point a mirror image of its own, held there, and points
/// on the line shared rather than copied.
pub(crate) fn mirror(sketch: &mut Sketch, curves: &[CurveId], line: CurveId) -> Result<usize, &'static str> {
    let Some(Shape::Line { a, b }) = sketch.curve(line).map(|c| c.shape.clone()) else {
        return Err("Click a line to mirror across");
    };
    let curves: Vec<CurveId> = curves.iter().copied().filter(|&c| c != line).collect();
    if curves.is_empty() {
        return Err("Select the curves to mirror first");
    }
    let (p, q) = (sketch.at(a), sketch.at(b));
    let length = gap(p, q).max(1e-12);
    let d = [(q[0] - p[0]) / length, (q[1] - p[1]) / length];
    let image = |x: [f64; 2]| {
        let along = (x[0] - p[0]) * d[0] + (x[1] - p[1]) * d[1];
        let foot = [p[0] + d[0] * along, p[1] + d[1] * along];
        [2.0 * foot[0] - x[0], 2.0 * foot[1] - x[1]]
    };
    let own: Vec<PointId> = curves.iter().filter_map(|&c| sketch.curve(c)).flat_map(|c| c.shape.points()).collect();
    let mut points = HashMap::new();
    for id in own {
        let at = sketch.at(id);
        if points.contains_key(&id) || gap(image(at), at) < 1e-6 {
            continue;
        }
        let copy = sketch.add_point(image(at));
        points.insert(id, copy);
        sketch.push_constraint(ConstraintKind::Symmetric { a: id, b: copy, line });
    }
    for &id in &curves {
        let Some(curve) = sketch.curve(id).cloned() else { continue };
        let copy = sketch.add_curve(remapped(&curve.shape, &points, true), curve.construction);
        // A circle's radius is its own, so only a constraint keeps the image the same size.
        if matches!(curve.shape, Shape::Circle { .. }) {
            sketch.push_constraint(ConstraintKind::Equal { a: id, b: copy });
        }
    }
    Ok(curves.len())
}

/// Copies of `curves` along the step from `from` to `to`, as many as make `settings`' count.
pub(crate) fn linear_pattern(
    sketch: &mut Sketch,
    curves: &[CurveId],
    from: [f64; 2],
    to: [f64; 2],
    settings: &Settings,
) {
    let step = [to[0] - from[0], to[1] - from[1]];
    for k in 1..settings.copies() {
        let k = k as f64;
        copy(sketch, curves, &|p| [p[0] + step[0] * k, p[1] + step[1] * k], false);
    }
}

/// Copies of `curves` round `centre`, spread over `settings`' angle: evenly all round for a full turn,
/// else the last copy at the far end of the spread.
pub(crate) fn circular_pattern(sketch: &mut Sketch, curves: &[CurveId], centre: [f64; 2], settings: &Settings) {
    let n = settings.copies();
    let spread = settings.pattern_angle.clamp(1.0, 360.0).to_radians();
    let step = if spread >= std::f64::consts::TAU - 1e-9 { spread / n as f64 } else { spread / (n - 1) as f64 };
    for k in 1..n {
        let turn = step * k as f64;
        copy(sketch, curves, &|p| turned(p, centre, turn), true);
    }
}

fn turned(p: [f64; 2], centre: [f64; 2], angle: f64) -> [f64; 2] {
    let (sin, cos) = angle.sin_cos();
    let (x, y) = (p[0] - centre[0], p[1] - centre[1]);
    [centre[0] + x * cos - y * sin, centre[1] + x * sin + y * cos]
}

/// How `Move`, `Rotate` or `Scale` places a point, from its clicks: the shift, the turn about the first
/// click, or the growth about it; `None` for clicks that make no change.
pub(crate) fn placing(draw: Draw, clicks: &[[f64; 2]]) -> Option<(Box<dyn Fn([f64; 2]) -> [f64; 2]>, f64)> {
    match draw {
        Draw::Move => {
            let shift = [clicks[1][0] - clicks[0][0], clicks[1][1] - clicks[0][1]];
            Some((Box::new(move |p| [p[0] + shift[0], p[1] + shift[1]]), 1.0))
        }
        Draw::Rotate => {
            let centre = clicks[0];
            let turn = angle_of(centre, clicks[2]) - angle_of(centre, clicks[1]);
            (gap(centre, clicks[1]) > 1e-9).then(|| (Box::new(move |p| turned(p, centre, turn)) as Box<_>, 1.0))
        }
        Draw::Scale => {
            let centre = clicks[0];
            let from = gap(centre, clicks[1]);
            let factor = gap(centre, clicks[2]) / from.max(1e-12);
            (from > 1e-9 && factor > 1e-6).then(|| {
                let place = move |p: [f64; 2]| {
                    [centre[0] + (p[0] - centre[0]) * factor, centre[1] + (p[1] - centre[1]) * factor]
                };
                (Box::new(place) as Box<_>, factor)
            })
        }
        _ => None,
    }
}

/// Move `points` (and the sizes of `curves`) by `draw`'s clicks. Fixes go along, sizes and lengths
/// among them grow with a scale, and a turn lets go of what held them square to the axes.
pub(crate) fn transform(
    sketch: &mut Sketch,
    draw: Draw,
    (curves, points): (&[CurveId], &[PointId]),
    clicks: &[[f64; 2]],
) -> bool {
    let Some((place, factor)) = placing(draw, clicks) else { return false };
    for point in sketch.points.iter_mut().filter(|p| points.contains(&p.id)) {
        point.at = place(point.at);
    }
    for curve in sketch.curves.iter_mut().filter(|c| curves.contains(&c.id)) {
        if let Some(size) = curve.shape.own_size_mut() {
            *size *= factor;
        }
    }
    let inside = |k: &ConstraintKind| {
        k.points().iter().all(|p| points.contains(p)) && k.curves().iter().all(|c| curves.contains(c))
    };
    if draw == Draw::Rotate {
        sketch.constraints.retain(|c| !(square_to_axes(&c.kind) && inside(&c.kind)));
    }
    for constraint in &mut sketch.constraints {
        let held = inside(&constraint.kind);
        match &mut constraint.kind {
            ConstraintKind::Fix { point, at } if points.contains(point) => *at = place(*at),
            kind if held && !kind.is_angle() => {
                if let Some(value) = kind.value_mut() {
                    *value *= factor;
                }
            }
            _ => {}
        }
    }
    true
}
