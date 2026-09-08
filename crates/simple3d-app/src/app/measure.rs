//! The measure tool: the points it holds and the distance between them.

use super::*;
use simple3d_geom::Vec3;

/// One end of a measurement: where it is, and what kind of feature it caught, so
/// the readout can say "vertex" or "face centre" and the marker can say the
/// point was snapped rather than dropped on the surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeasurePoint {
    pub at: Vec3,
    pub kind: Option<crate::snap::FeatureKind>,
}

/// The measure tool's state (issue 69): whether it is holding the pointer, and
/// the one or two points picked so far.
///
/// The points stay put until the tool is dismissed -- the readout is meant to be
/// left on screen and looked at while the model is turned -- so this is its own
/// small piece of state rather than something recomputed each frame. Picking a
/// third point starts a fresh measurement from it.
#[derive(Clone, Debug, Default)]
pub struct Measure {
    pub active: bool,
    pub points: Vec<MeasurePoint>,
}

impl Measure {
    /// Add a point, beginning a new measurement once a pair is complete so the
    /// tool flows from one span to the next without a clear in between.
    pub fn add(&mut self, point: MeasurePoint) {
        if self.points.len() >= 2 {
            self.points.clear();
        }
        self.points.push(point);
    }

    pub fn clear(&mut self) {
        self.points.clear();
    }

    /// Move one end of the span, or place it if it is not down yet. What the
    /// editable start and end fields in the property panel write to (issue 78).
    ///
    /// An end can only be placed once the one before it is: an end with no start
    /// is not half a measurement, it is a point with nothing to measure to.
    pub fn set_point(&mut self, index: usize, at: Vec3) {
        // Typing a coordinate is placing the point exactly, so whatever feature
        // it once caught is no longer what it is.
        let placed = MeasurePoint { at, kind: None };
        if index < self.points.len() {
            self.points[index] = placed;
        } else if index == self.points.len() && index < 2 {
            self.points.push(placed);
        }
    }

    /// The finished span, once both ends are placed.
    pub fn span(&self) -> Option<(MeasurePoint, MeasurePoint)> {
        match self.points.as_slice() {
            [a, b] => Some((*a, *b)),
            _ => None,
        }
    }
}

/// The numbers a span reads out: the straight-line distance, the per-axis delta,
/// the angle above the ground plane and the compass bearing around Z (issue 69).
///
/// The angle is split into these two because a single number cannot place a line
/// in space: inclination says how steep it is, bearing which way it runs, and
/// together they are the direction the delta points.
pub struct Measurement {
    pub distance: f64,
    pub delta: Vec3,
    pub inclination_deg: f64,
    pub bearing_deg: f64,
}

impl Measurement {
    pub fn between(a: Vec3, b: Vec3) -> Measurement {
        let delta = b - a;
        let distance = delta.length();
        let horizontal = (delta.x * delta.x + delta.y * delta.y).sqrt();
        // Inclination above the XY plane: 0 for a level span, +/-90 for a
        // vertical one. Undefined for a zero-length span, which reads as level.
        let inclination_deg = if distance < 1e-9 { 0.0 } else { delta.z.atan2(horizontal).to_degrees() };
        // Bearing around Z, measured from +X towards +Y, so it agrees with how a
        // yaw is read. A span with no horizontal run has no bearing; 0 is as good
        // as any and does not mislead because the inclination is then +/-90.
        let bearing_deg = if horizontal < 1e-9 { 0.0 } else { delta.y.atan2(delta.x).to_degrees() };
        Measurement { distance, delta, inclination_deg, bearing_deg }
    }
}

impl App {
    /// Take the last placed end back off, leaving the one before it in place: a
    /// right-click in the viewport, for a point put down in the wrong place.
    ///
    /// Unset rather than moved: an end that is gone is placed again by the next
    /// click, which is what "reset to unset" has to mean for a tool whose next
    /// click always places the next end.
    pub fn measure_unplace(&mut self) {
        match self.measure.points.pop() {
            Some(_) if self.measure.points.is_empty() => {
                self.status = Status::Info("Measure: the start is unset again".into())
            }
            Some(_) => self.status = Status::Info("Measure: click the second feature".into()),
            None => self.status = Status::Info("Measure: nothing placed to take back".into()),
        }
    }

    /// Record a measure click, and say what the span reads once both ends are
    /// down (issue 69).
    pub fn measure_click(&mut self, point: MeasurePoint) {
        self.measure.add(point);
        if let Some((a, b)) = self.measure.span() {
            let m = Measurement::between(a.at, b.at);
            // With the unit: this line sits in the same bar as "20 x 20 x 20 mm"
            // and "Grid 10 mm", and a bare number among them says nothing at all
            // in a drawing whose unit is centimetres.
            self.status = Status::Info(format!(
                "Distance {} {}  \u{00B7}  incline {}\u{00B0}",
                simple3d_core::unit::format_length(m.distance, self.unit()),
                self.unit().suffix(),
                simple3d_core::unit::format_angle(m.inclination_deg),
            ));
        } else {
            self.status = Status::Info("Measure: click the second feature".into());
        }
    }
}
