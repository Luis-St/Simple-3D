//! The drawing tools' newer variants: centre lines, turned and rounded rectangles, circles across a
//! diameter, polygons about their sides, slots and ellipses. Each is built from its clicks in one go,
//! so the gesture's preview builds the same thing into a copy of the sketch.

use super::drawing::{point_for, rectangle, regular};
use super::settings::Settings;
use super::shapes::{angle_of, gap, on_circle};
use super::*;
use simple3d_sketch::{ConstraintKind, Ellipse, Shape};
use std::f64::consts::PI;

/// Lengths shorter than this, in millimetres, are nothing: two clicks on one place.
const TINY: f64 = 1e-6;

fn along(p: [f64; 2], d: [f64; 2], t: f64) -> [f64; 2] {
    [p[0] + d[0] * t, p[1] + d[1] * t]
}

/// The unit direction from `a` to `b` and the quarter turn left of it; `None` when they meet.
fn frame(a: [f64; 2], b: [f64; 2]) -> Option<([f64; 2], [f64; 2])> {
    let length = gap(a, b);
    (length > TINY).then(|| {
        let d = [(b[0] - a[0]) / length, (b[1] - a[1]) / length];
        (d, [-d[1], d[0]])
    })
}

/// How many clicks a variant here takes, or `None` for a tool drawn elsewhere.
pub(crate) fn clicks_needed(draw: Draw) -> Option<usize> {
    Some(match draw {
        Draw::CentreLine | Draw::RoundedRectangle | Draw::TwoPointCircle | Draw::CircumscribedPolygon => 2,
        Draw::ThreePointRectangle | Draw::Slot | Draw::OverallSlot | Draw::Ellipse => 3,
        Draw::ArcSlot | Draw::EllipseArc => 4,
        _ => return None,
    })
}

/// Build variant `draw` from its `clicks` into `sketch`; `None`, with the sketch half changed, when
/// the clicks make nothing (callers build into a copy, or through `change` on a clone).
pub(crate) fn build(draw: Draw, sketch: &mut Sketch, clicks: &[Click], settings: &Settings) -> Option<()> {
    let at = |i: usize| clicks[i].at;
    match draw {
        Draw::CentreLine => {
            let (mid, end) = (clicks[0], clicks[1]);
            (gap(mid.at, end.at) > TINY).then_some(())?;
            let middle = point_for(sketch, mid);
            let b = point_for(sketch, end);
            let a = sketch.add_point([2.0 * mid.at[0] - end.at[0], 2.0 * mid.at[1] - end.at[1]]);
            let line = sketch.add_curve(Shape::Line { a, b }, false);
            sketch.push_constraint(ConstraintKind::Midpoint { point: middle, line });
        }
        Draw::ThreePointRectangle => {
            let (_, n) = frame(at(0), at(1))?;
            let w = n[0] * (at(2)[0] - at(0)[0]) + n[1] * (at(2)[1] - at(0)[1]);
            (w.abs() > TINY).then_some(())?;
            let (p0, p1) = (point_for(sketch, clicks[0]), point_for(sketch, clicks[1]));
            let p2 = sketch.add_point(along(at(1), n, w));
            let p3 = sketch.add_point(along(at(0), n, w));
            let corners = [p0, p1, p2, p3];
            let sides: Vec<CurveId> = (0..4)
                .map(|i| sketch.add_curve(Shape::Line { a: corners[i], b: corners[(i + 1) % 4] }, false))
                .collect();
            sketch.push_constraint(ConstraintKind::Perpendicular { a: sides[0], b: sides[1] });
            sketch.push_constraint(ConstraintKind::Parallel { a: sides[0], b: sides[2] });
            sketch.push_constraint(ConstraintKind::Parallel { a: sides[1], b: sides[3] });
        }
        Draw::RoundedRectangle => {
            let (w, h) = ((at(1)[0] - at(0)[0]).abs(), (at(1)[1] - at(0)[1]).abs());
            (w > TINY && h > TINY).then_some(())?;
            let corners = rectangle(sketch, clicks[0], clicks[1]);
            let radius = settings.fillet_radius.min(0.5 * w.min(h) - 1e-3);
            if radius > TINY {
                round_corners(sketch, &corners, radius);
            }
        }
        Draw::TwoPointCircle => {
            let radius = gap(at(0), at(1)) / 2.0;
            (radius > TINY).then_some(())?;
            let center = sketch.add_point([(at(0)[0] + at(1)[0]) / 2.0, (at(0)[1] + at(1)[1]) / 2.0]);
            let circle = sketch.add_curve(Shape::Circle { center, radius }, false);
            for point in clicks.iter().filter_map(|c| c.point) {
                sketch.push_constraint(ConstraintKind::OnCurve { point, curve: circle });
            }
        }
        Draw::CircumscribedPolygon => {
            let sides = settings.sides();
            let apothem = gap(at(0), at(1));
            (apothem > TINY).then_some(())?;
            let half = PI / sides as f64;
            let corner = on_circle(at(0), apothem / half.cos(), angle_of(at(0), at(1)) + half);
            regular(sketch, clicks[0], Click { at: corner, point: None }, sides);
        }
        Draw::Slot | Draw::OverallSlot => {
            let (d, n) = frame(at(0), at(1))?;
            let r = (n[0] * (at(2)[0] - at(0)[0]) + n[1] * (at(2)[1] - at(0)[1])).abs();
            (r > TINY).then_some(())?;
            let (c1, c2) = match draw {
                Draw::Slot => (point_for(sketch, clicks[0]), point_for(sketch, clicks[1])),
                _ => {
                    (gap(at(0), at(1)) > 2.0 * r + TINY).then_some(())?;
                    (sketch.add_point(along(at(0), d, r)), sketch.add_point(along(at(1), d, -r)))
                }
            };
            straight_slot(sketch, c1, c2, r);
        }
        Draw::ArcSlot => {
            let o = at(0);
            let radius = gap(o, at(1));
            let r = (gap(o, at(3)) - radius).abs();
            (radius > TINY && r > TINY && r < radius - TINY && gap(o, at(2)) > TINY).then_some(())?;
            let end = Click { at: on_circle(o, radius, angle_of(o, at(2))), point: None };
            (gap(end.at, at(1)) > TINY).then_some(())?;
            let (centre, start) = (point_for(sketch, clicks[0]), point_for(sketch, clicks[1]));
            let end = point_for(sketch, end);
            arc_slot(sketch, centre, start, end, r);
        }
        Draw::Ellipse | Draw::EllipseArc => {
            let (_, n) = frame(at(0), at(1))?;
            let minor = match draw {
                Draw::Ellipse => (n[0] * (at(2)[0] - at(0)[0]) + n[1] * (at(2)[1] - at(0)[1])).abs(),
                _ => Ellipse::minor_through(at(0), at(1), at(2))?,
            };
            (minor > TINY).then_some(())?;
            let (center, major) = (point_for(sketch, clicks[0]), point_for(sketch, clicks[1]));
            sketch.add_curve(Shape::Line { a: center, b: major }, true);
            if draw == Draw::Ellipse {
                sketch.add_curve(Shape::Ellipse { center, major, minor }, false);
            } else {
                let e = Ellipse::new(at(0), at(1), minor);
                let end = match clicks[3].point {
                    Some(_) => clicks[3],
                    None => Click { at: e.at(e.angle_of(at(3))), point: None },
                };
                (gap(end.at, at(2)) > TINY).then_some(())?;
                let (start, end) = (point_for(sketch, clicks[2]), point_for(sketch, end));
                sketch.add_curve(Shape::EllipseArc { center, major, minor, start, end }, false);
            }
        }
        _ => return None,
    }
    Some(())
}

/// Round the four `corners` of a rectangle at `radius`, one radius dimension with the other arcs
/// held equal to it, so the size is typed once.
fn round_corners(sketch: &mut Sketch, corners: &[PointId; 4], radius: f64) {
    let arcs: Vec<CurveId> =
        corners.iter().filter_map(|&corner| super::modify::fillet(sketch, corner, radius).ok()).collect();
    let Some((&first, rest)) = arcs.split_first() else { return };
    sketch.constraints.retain(|c| !matches!(c.kind, ConstraintKind::Radius { curve, .. } if rest.contains(&curve)));
    for &arc in rest {
        sketch.push_constraint(ConstraintKind::Equal { a: first, b: arc });
    }
}

/// A slot round centres `c1` and `c2`, its ends of radius `r`: two sides tangent to two equal end
/// arcs, and a helper line between the centres to dimension its length on.
fn straight_slot(sketch: &mut Sketch, c1: PointId, c2: PointId, r: f64) {
    let (p, q) = (sketch.at(c1), sketch.at(c2));
    let Some((_, n)) = frame(p, q) else { return };
    let corners = [along(p, n, r), along(q, n, r), along(q, n, -r), along(p, n, -r)].map(|at| sketch.add_point(at));
    let top = sketch.add_curve(Shape::Line { a: corners[0], b: corners[1] }, false);
    let bottom = sketch.add_curve(Shape::Line { a: corners[2], b: corners[3] }, false);
    // Counter-clockwise round each end, from the side below the axis to the one above at the far end.
    let far = sketch.add_curve(Shape::Arc { center: c2, start: corners[2], end: corners[1] }, false);
    let near = sketch.add_curve(Shape::Arc { center: c1, start: corners[0], end: corners[3] }, false);
    for (line, arc) in [(top, far), (bottom, far), (top, near), (bottom, near)] {
        sketch.push_constraint(ConstraintKind::Tangent { a: line, b: arc, inside: false });
    }
    sketch.push_constraint(ConstraintKind::Equal { a: near, b: far });
    sketch.add_curve(Shape::Line { a: c1, b: c2 }, true);
}

/// A slot bent along the arc about `centre` from `start` to `end`, `r` either side of it: an outer
/// and an inner arc about the same centre, joined by round ends about the arc's ends. No helper arc
/// runs along its middle: the four tangencies already keep the ends' centres on it, and the helper
/// would only repeat them.
fn arc_slot(sketch: &mut Sketch, centre: PointId, start: PointId, end: PointId, r: f64) {
    let (o, s, e) = (sketch.at(centre), sketch.at(start), sketch.at(end));
    let radius = gap(o, s);
    let (a0, a1) = (angle_of(o, s), angle_of(o, e));
    let mut point = |angle: f64, out: f64| sketch.add_point(on_circle(o, radius + out, angle));
    let (outer_start, outer_end) = (point(a0, r), point(a1, r));
    let (inner_start, inner_end) = (point(a0, -r), point(a1, -r));
    let outer = sketch.add_curve(Shape::Arc { center: centre, start: outer_start, end: outer_end }, false);
    let inner = sketch.add_curve(Shape::Arc { center: centre, start: inner_start, end: inner_end }, false);
    // The start's end bulges back against the arc's run, the end's forward with it.
    let back = sketch.add_curve(Shape::Arc { center: start, start: inner_start, end: outer_start }, false);
    let front = sketch.add_curve(Shape::Arc { center: end, start: outer_end, end: inner_end }, false);
    for (a, b) in [(back, outer), (back, inner), (front, outer), (front, inner)] {
        sketch.push_constraint(ConstraintKind::Tangent { a, b, inside: false });
    }
}
