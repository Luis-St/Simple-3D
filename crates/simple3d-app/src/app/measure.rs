//! The measure tool: the points it holds and the distance between them.

use super::*;
use simple3d_geom::Vec3;

/// One end of a measurement and the feature kind it caught, for the readout and marker.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeasurePoint {
    pub at: Vec3,
    pub kind: Option<crate::snap::FeatureKind>,
}

/// The measure tool's state (issue 69): whether it holds the pointer and the points picked. The
/// points stay until dismissed; a third point starts a new measurement.
#[derive(Clone, Debug, Default)]
pub struct Measure {
    pub active: bool,
    pub points: Vec<MeasurePoint>,
}

impl Measure {
    /// Add a point, starting a new measurement after a complete pair.
    pub fn add(&mut self, point: MeasurePoint) {
        if self.points.len() >= 2 {
            self.points.clear();
        }
        self.points.push(point);
    }

    pub fn clear(&mut self) {
        self.points.clear();
    }

    /// Move or place one end of the span, from the editable fields (issue 78). An end can only be
    /// placed after the one before it.
    pub fn set_point(&mut self, index: usize, at: Vec3) {
        // A typed coordinate is exact, so it no longer carries a caught feature.
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

/// A span's readout (issue 69): distance, per-axis delta, inclination above the ground and bearing
/// around Z, which together give the direction.
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
        // Inclination above XY: 0 level, +/-90 vertical; a zero-length span reads as level.
        let inclination_deg = if distance < 1e-9 { 0.0 } else { delta.z.atan2(horizontal).to_degrees() };
        // Bearing around Z from +X towards +Y, like yaw; 0 when vertical, where inclination says +/-90.
        let bearing_deg = if horizontal < 1e-9 { 0.0 } else { delta.y.atan2(delta.x).to_degrees() };
        Measurement { distance, delta, inclination_deg, bearing_deg }
    }
}

impl App {
    /// Remove the last placed end (a viewport right-click); the next click places it again.
    pub fn measure_unplace(&mut self) {
        match self.measure.points.pop() {
            Some(_) if self.measure.points.is_empty() => {
                self.status = Status::Info("Measure: the start is unset again".into())
            }
            Some(_) => self.status = Status::Info("Measure: click the second feature".into()),
            None => self.status = Status::Info("Measure: nothing placed to take back".into()),
        }
    }

    /// Record a measure click, and report the span once both ends are down (issue 69).
    pub fn measure_click(&mut self, point: MeasurePoint) {
        self.measure.add(point);
        if let Some((a, b)) = self.measure.span() {
            let m = Measurement::between(a.at, b.at);
            // With the unit, since a bare number among the status bar's readings is ambiguous.
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
