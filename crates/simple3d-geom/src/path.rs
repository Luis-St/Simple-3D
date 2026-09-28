//! A path through space: a polyline, open or closed, measured along its length (issue 70).
//!
//! Objects are spread along a path by arc length, so a path of uneven segments still spaces them
//! evenly. A path is drawn point by point or chained from picked body edges ([`Path::from_edges`]),
//! and a picked edge can bring the smooth run it belongs to ([`smooth_run`]), since a round edge is
//! dozens of short segments.

mod chain;
pub use chain::smooth_run;
#[cfg(test)]
mod tests;

use crate::Vec3;

/// Two points closer than this are one, in millimetres: far below any model's detail, far above
/// the rounding of an edge read back from a welded mesh.
pub const SAME_POINT: f64 = 1e-4;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path {
    pub points: Vec<Vec3>,
    /// The last point runs back to the first.
    pub closed: bool,
}

impl Path {
    /// Every segment in order, including the closing one.
    pub fn segments(&self) -> Vec<(Vec3, Vec3)> {
        let n = self.points.len();
        if n < 2 {
            return Vec::new();
        }
        let last = if self.closed && n > 2 { n } else { n - 1 };
        (0..last).map(|i| (self.points[i], self.points[(i + 1) % n])).collect()
    }

    pub fn length(&self) -> f64 {
        self.segments().iter().map(|(a, b)| (*b - *a).length()).sum()
    }

    /// The point `s` millimetres along the path, clamped to it, with the unit direction of the segment
    /// it lies on. A point on a corner takes the segment leaving it, so a path's start faces along its
    /// first segment. `None` for a path without length.
    pub fn at(&self, s: f64) -> Option<(Vec3, Vec3)> {
        let segments: Vec<(Vec3, Vec3, f64)> =
            self.segments().into_iter().map(|(a, b)| (a, b, (b - a).length())).filter(|s| s.2 > SAME_POINT).collect();
        let total: f64 = segments.iter().map(|s| s.2).sum();
        if segments.is_empty() || total <= SAME_POINT {
            return None;
        }
        let mut left = s.clamp(0.0, total);
        for (index, &(a, b, length)) in segments.iter().enumerate() {
            if left < length || index + 1 == segments.len() {
                let t = (left / length).clamp(0.0, 1.0);
                return Some((a.lerp(b, t), (b - a) * (1.0 / length)));
            }
            left -= length;
        }
        None
    }

    /// How far along the path the point nearest `p` lies, for putting objects in path order.
    pub fn distance_along(&self, p: Vec3) -> f64 {
        let mut best = (f64::INFINITY, 0.0);
        let mut walked = 0.0;
        for (a, b) in self.segments() {
            let along = b - a;
            let length = along.length();
            let t = if length > SAME_POINT { ((p - a).dot(along) / (length * length)).clamp(0.0, 1.0) } else { 0.0 };
            let away = (a.lerp(b, t) - p).length();
            if away < best.0 {
                best = (away, walked + t * length);
            }
            walked += length;
        }
        best.1
    }

    /// Where `count` objects go to be evenly spread, as distances along the path. An open path puts
    /// the first and last on its ends; a closed one spaces them all round, since its ends meet. One
    /// object goes on the start.
    pub fn spread(&self, count: usize) -> Vec<f64> {
        let length = self.length();
        match count {
            0 => Vec::new(),
            1 => vec![0.0],
            _ if self.closed => (0..count).map(|i| length * i as f64 / count as f64).collect(),
            _ => (0..count).map(|i| length * i as f64 / (count - 1) as f64).collect(),
        }
    }

    /// Picked edges joined into one path, in the order they connect. Edges sharing an end are chained;
    /// a gap is bridged straight to the nearest free edge, so any picking still gives one path. A chain
    /// that comes back to its start is closed.
    pub fn from_edges(edges: &[(Vec3, Vec3)]) -> Path {
        chain::chain(edges)
    }
}
