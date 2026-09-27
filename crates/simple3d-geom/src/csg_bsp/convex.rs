//! A convex body as half-spaces, for cheap containment tests that avoid splitting.

use super::*;
use crate::vec3::Vec3;

/// A convex solid stated directly rather than as a BSP chain. Clipping against the chain splits by
/// every infinite face plane (103 million splits for a plate against a 256-segment cap), whereas
/// `clip_convex` only cuts by faces whose boxes come near a polygon, which is exact.
pub(crate) struct ConvexBody {
    /// The distinct face planes in `chain` order, always outward; inversion is tracked separately.
    pub(super) planes: Vec<Plane>,
    /// Which plane each polygon lies in, indexed as `surface` indexes them.
    pub(super) plane_of: Vec<u32>,
    /// The body's box, so far-off points cost one test.
    pub(super) bounds: (Vec3, Vec3),
    /// Whether `invert` made this the complement; only the solid side changes.
    pub(super) inverted: bool,
}

impl ConvexBody {
    pub(super) fn new(polygons: &[Polygon], groups: &[Vec<usize>]) -> ConvexBody {
        let mut plane_of = vec![0u32; polygons.len()];
        let mut planes = Vec::with_capacity(groups.len());
        for (g, group) in groups.iter().enumerate() {
            planes.push(polygons[group[0]].plane);
            for &i in group {
                plane_of[i] = g as u32;
            }
        }
        let mut lo = polygons[0].bounds.0;
        let mut hi = polygons[0].bounds.1;
        for p in &polygons[1..] {
            lo = lo.min(p.bounds.0);
            hi = hi.max(p.bounds.1);
        }
        ConvexBody { planes, plane_of, bounds: (lo, hi), inverted: false }
    }

    /// Whether `point` is inside, behind every face. The only place far planes are consulted; box and
    /// early exits settle most points.
    pub(super) fn contains(&self, point: Vec3) -> bool {
        if !boxes_meet((point, point), self.bounds) {
            return false;
        }
        self.planes.iter().all(|pl| pl.normal.dot(point) - pl.w <= 0.0)
    }
}
