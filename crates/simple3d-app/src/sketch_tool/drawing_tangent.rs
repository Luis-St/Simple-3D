//! The tangent arc: an arc leaving the end of a line or arc in the direction that curve arrives in,
//! held tangent to it, and the next one leaving the arc's own end, until the chain is ended.

use super::*;
use simple3d_sketch::{ConstraintKind, Shape};

/// The curve ending on `point` that an arc from there runs on from -- the newest line or arc, so a
/// chain follows its last piece -- and the direction it arrives in.
pub(crate) fn arriving(sketch: &Sketch, point: PointId) -> Option<(CurveId, [f64; 2])> {
    let p = sketch.at(point);
    sketch.curves.iter().rev().find_map(|curve| {
        let direction = match curve.shape {
            Shape::Line { a, b } if b == point => [p[0] - sketch.at(a)[0], p[1] - sketch.at(a)[1]],
            Shape::Line { a, b } if a == point => [p[0] - sketch.at(b)[0], p[1] - sketch.at(b)[1]],
            Shape::Arc { center, start, end } if start == point || end == point => {
                let c = sketch.at(center);
                let r = [p[0] - c[0], p[1] - c[1]];
                // Counter-clockwise travel arrives at the end turned left of the radius, and leaves
                // the start that way, so arriving at the start is the other way round.
                if end == point {
                    [-r[1], r[0]]
                } else {
                    [r[1], -r[0]]
                }
            }
            _ => return None,
        };
        let length = direction[0].hypot(direction[1]);
        (length > 1e-9).then(|| (curve.id, [direction[0] / length, direction[1] / length]))
    })
}

/// The arc from `from`, leaving in unit direction `heading`, to the click `to`, held tangent to
/// curve `after`; its far end, or `None` when the click lies straight ahead.
fn tangent_arc(
    sketch: &mut Sketch,
    from: PointId,
    (after, heading): (CurveId, [f64; 2]),
    to: Click,
) -> Option<PointId> {
    let p = sketch.at(from);
    let left = [-heading[1], heading[0]];
    let chord = [to.at[0] - p[0], to.at[1] - p[1]];
    let across = chord[0] * left[0] + chord[1] * left[1];
    let span = chord[0] * chord[0] + chord[1] * chord[1];
    if across.abs() < 1e-9 * span.sqrt().max(1.0) {
        return None;
    }
    // The centre is on the left (a left turn) or right of the heading, as far as makes the click lie
    // on the circle.
    let reach = span / (2.0 * across);
    let center = sketch.add_point([p[0] + left[0] * reach, p[1] + left[1] * reach]);
    let end = super::drawing::point_for(sketch, to);
    let shape = match across > 0.0 {
        true => Shape::Arc { center, start: from, end },
        false => Shape::Arc { center, start: end, end: from },
    };
    let arc = sketch.add_curve(shape, false);
    sketch.push_constraint(ConstraintKind::Tangent { a: after, b: arc, inside: false });
    Some(end)
}

impl SketchTool {
    /// A tangent arc click: the first must land on the end of a line or arc, each later one ends an
    /// arc and starts the next from there.
    pub(crate) fn tangent_click(&mut self, click: Click) {
        let Some(start) = self.clicks.first().and_then(|c| c.point) else {
            if click.point.is_some_and(|p| arriving(&self.sketch, p).is_some()) {
                self.clicks = vec![click];
            }
            return;
        };
        let Some(heading) = arriving(&self.sketch, start) else {
            self.clicks.clear();
            return;
        };
        let mut sketch = self.sketch.clone();
        if let Some(end) = tangent_arc(&mut sketch, start, heading, click) {
            self.change(|s| *s = sketch);
            self.clicks = vec![Click { at: self.sketch.at(end), point: Some(end) }];
        }
    }

    /// The tangent arc the next click would put down, built into `sketch`, a copy, for the preview.
    pub(crate) fn tangent_preview(&self, sketch: &mut Sketch, to: Click) -> bool {
        let Some(start) = self.clicks.first().and_then(|c| c.point) else { return false };
        let Some(heading) = arriving(sketch, start) else { return false };
        tangent_arc(sketch, start, heading, to).is_some()
    }
}
