//! The drawing tools: what each click of a gesture adds to the sketch.

use super::shapes::{arc_through, centred_rectangle, circle_through, gap, polygon};
use super::*;
use simple3d_sketch::{ConstraintKind, Shape};

/// Two clicks this close, in millimetres, are one place: a double click, not a line.
const SAME_PLACE: f64 = 1e-6;
/// The most sides the polygon tool draws.
pub(crate) const MAX_SIDES: u32 = 64;

/// The sketch point a click stands for: the one it landed on, else a new one where it fell.
fn point_for(sketch: &mut Sketch, click: Click) -> PointId {
    click.point.filter(|&id| sketch.point(id).is_some()).unwrap_or_else(|| sketch.add_point(click.at))
}

impl SketchTool {
    /// Take one click of the current drawing tool.
    pub(crate) fn click(&mut self, click: Click) {
        let last = self.clicks.last().copied();
        if last.is_some_and(|last| gap(last.at, click.at) < SAME_PLACE) {
            return;
        }
        match self.draw {
            Draw::Line => match last {
                Some(start) => {
                    let (a, b) = self.change(|sketch| {
                        let (a, b) = (point_for(sketch, start), point_for(sketch, click));
                        sketch.add_curve(Shape::Line { a, b }, false);
                        (a, b)
                    });
                    // The chain remembers its first point, and a line back onto it closes the chain.
                    let first =
                        if self.clicks.len() >= 2 { self.clicks[0] } else { Click { at: start.at, point: Some(a) } };
                    let closed = Some(b) == first.point;
                    self.clicks = if closed { Vec::new() } else { vec![first, Click { at: click.at, point: Some(b) }] };
                }
                None => self.clicks.push(click),
            },
            Draw::Rectangle => match last {
                Some(corner) => {
                    self.clicks.clear();
                    let (lo, hi) = (corner.at, click.at);
                    if (lo[0] - hi[0]).abs() < SAME_PLACE || (lo[1] - hi[1]).abs() < SAME_PLACE {
                        return;
                    }
                    self.change(|sketch| rectangle(sketch, corner, click));
                }
                None => self.clicks.push(click),
            },
            Draw::CenterRectangle => match last {
                Some(centre) => {
                    self.clicks.clear();
                    if (centre.at[0] - click.at[0]).abs() < SAME_PLACE
                        || (centre.at[1] - click.at[1]).abs() < SAME_PLACE
                    {
                        return;
                    }
                    self.change(|sketch| centred(sketch, centre, click));
                }
                None => self.clicks.push(click),
            },
            Draw::ThreePointCircle => {
                if self.clicks.len() < 2 {
                    self.clicks.push(click);
                    return;
                }
                let (a, b) = (self.clicks[0], self.clicks[1]);
                self.clicks.clear();
                let Some((centre, radius)) = circle_through(a.at, b.at, click.at) else { return };
                self.change(|sketch| {
                    let center = sketch.add_point(centre);
                    let circle = sketch.add_curve(Shape::Circle { center, radius }, false);
                    // Clicks on existing points keep the circle running through them.
                    for point in [a, b, click].into_iter().filter_map(|c| c.point) {
                        sketch.push_constraint(ConstraintKind::OnCurve { point, curve: circle });
                    }
                });
            }
            Draw::ThreePointArc => {
                if self.clicks.len() < 2 {
                    self.clicks.push(click);
                    return;
                }
                let (from, to) = (self.clicks[0], self.clicks[1]);
                self.clicks.clear();
                let Some((centre, start, _)) = arc_through(from.at, to.at, click.at) else { return };
                let (start, end) = if start == from.at { (from, to) } else { (to, from) };
                self.change(|sketch| {
                    let center = sketch.add_point(centre);
                    let (start, end) = (point_for(sketch, start), point_for(sketch, end));
                    sketch.add_curve(Shape::Arc { center, start, end }, false);
                });
            }
            Draw::Polygon => match last {
                Some(centre) => {
                    self.clicks.clear();
                    if gap(centre.at, click.at) < SAME_PLACE {
                        return;
                    }
                    let sides = self.polygon_sides.clamp(3, MAX_SIDES);
                    self.change(|sketch| regular(sketch, centre, click, sides));
                }
                None => self.clicks.push(click),
            },
            Draw::Circle => match last {
                Some(centre) => {
                    self.clicks.clear();
                    let radius = gap(centre.at, click.at);
                    self.change(|sketch| {
                        let center = point_for(sketch, centre);
                        sketch.add_curve(Shape::Circle { center, radius }, false);
                    });
                }
                None => self.clicks.push(click),
            },
            Draw::Arc => {
                if self.clicks.len() < 2 {
                    self.clicks.push(click);
                    return;
                }
                let (centre, start) = (self.clicks[0], self.clicks[1]);
                self.clicks.clear();
                let radius = gap(centre.at, start.at);
                let away = gap(centre.at, click.at);
                if radius < SAME_PLACE || away < SAME_PLACE {
                    return;
                }
                // A free end goes onto the circle the start set, toward the click.
                let end = match click.point {
                    Some(_) => click,
                    None => {
                        let d = [(click.at[0] - centre.at[0]) / away, (click.at[1] - centre.at[1]) / away];
                        Click { at: [centre.at[0] + d[0] * radius, centre.at[1] + d[1] * radius], point: None }
                    }
                };
                self.change(|sketch| {
                    let (center, start, end) =
                        (point_for(sketch, centre), point_for(sketch, start), point_for(sketch, end));
                    sketch.add_curve(Shape::Arc { center, start, end }, false);
                });
            }
            Draw::Spline | Draw::Bspline => {
                let first = self.clicks.first().copied();
                let onto_first = first.is_some_and(|first| {
                    (click.point.is_some() && click.point == first.point) || gap(first.at, click.at) < SAME_PLACE
                });
                if onto_first && self.clicks.len() >= 3 {
                    self.commit_spline(true);
                } else {
                    self.clicks.push(click);
                }
            }
            Draw::Point if click.point.is_none() => {
                self.change(|sketch| sketch.add_point(click.at));
            }
            _ => {}
        }
    }

    /// End the gesture in progress: an open spline is kept if it has two points, a line chain stops.
    pub(crate) fn end_gesture(&mut self) {
        if matches!(self.draw, Draw::Spline | Draw::Bspline) && self.clicks.len() >= 2 {
            self.commit_spline(false);
        }
        self.clicks.clear();
    }

    fn commit_spline(&mut self, closed: bool) {
        let clicks = std::mem::take(&mut self.clicks);
        let through = self.draw == Draw::Spline;
        self.change(|sketch| {
            let points: Vec<PointId> = clicks.iter().map(|&click| point_for(sketch, click)).collect();
            let shape = if through { Shape::Spline { points, closed } } else { Shape::Bspline { points, closed } };
            sketch.add_curve(shape, false);
        });
    }
}

/// A rectangle from corner `a` to corner `b`, kept square to the sketch's axes by its constraints.
fn rectangle(sketch: &mut Sketch, a: Click, b: Click) {
    let p0 = point_for(sketch, a);
    let p2 = point_for(sketch, b);
    let p1 = sketch.add_point([b.at[0], a.at[1]]);
    let p3 = sketch.add_point([a.at[0], b.at[1]]);
    let corners = [p0, p1, p2, p3];
    for i in 0..4 {
        let line = sketch.add_curve(Shape::Line { a: corners[i], b: corners[(i + 1) % 4] }, false);
        let kind = if i % 2 == 0 { ConstraintKind::Horizontal { line } } else { ConstraintKind::Vertical { line } };
        sketch.push_constraint(kind);
    }
}

/// A rectangle about the centre click out to the corner click: square to the sketch's axes, with
/// helper diagonals through a centre point that keeps it centred.
fn centred(sketch: &mut Sketch, centre: Click, corner: Click) {
    let at = centred_rectangle(centre.at, corner.at);
    let mid = point_for(sketch, centre);
    let first = point_for(sketch, corner);
    let corners: Vec<PointId> = std::iter::once(first).chain(at[1..].iter().map(|&p| sketch.add_point(p))).collect();
    for i in 0..4 {
        let line = sketch.add_curve(Shape::Line { a: corners[i], b: corners[(i + 1) % 4] }, false);
        let kind = if i % 2 == 0 { ConstraintKind::Horizontal { line } } else { ConstraintKind::Vertical { line } };
        sketch.push_constraint(kind);
    }
    let diagonal = sketch.add_curve(Shape::Line { a: corners[0], b: corners[2] }, true);
    sketch.add_curve(Shape::Line { a: corners[1], b: corners[3] }, true);
    sketch.push_constraint(ConstraintKind::Midpoint { point: mid, line: diagonal });
}

/// A regular polygon of `sides` about the centre click, a corner at the corner click: its corners
/// held on a helper circle and its sides equal, so it stays regular as it is dragged.
fn regular(sketch: &mut Sketch, centre: Click, corner: Click, sides: u32) {
    let at = polygon(centre.at, corner.at, sides);
    let center = point_for(sketch, centre);
    let first = point_for(sketch, corner);
    let corners: Vec<PointId> = std::iter::once(first).chain(at[1..].iter().map(|&p| sketch.add_point(p))).collect();
    let radius = gap(centre.at, corner.at);
    let circle = sketch.add_curve(Shape::Circle { center, radius }, true);
    let n = corners.len();
    let lines: Vec<CurveId> =
        (0..n).map(|i| sketch.add_curve(Shape::Line { a: corners[i], b: corners[(i + 1) % n] }, false)).collect();
    for &point in &corners {
        sketch.push_constraint(ConstraintKind::OnCurve { point, curve: circle });
    }
    // Every side made equal to the first; with the corners on the circle that leaves it free only to
    // move, grow and turn.
    for &b in &lines[1..] {
        sketch.push_constraint(ConstraintKind::Equal { a: lines[0], b });
    }
}
