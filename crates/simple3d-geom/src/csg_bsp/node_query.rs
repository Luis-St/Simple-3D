//! Point and ray classification against a built tree.

use super::*;
use crate::vec3::Vec3;

/// Tree walks use an explicit stack, since a convex body's tree is a deep chain (see `node_build`).
impl BspNode {
    /// Whether `point` is outside this body, and so kept by a clip. A convex body's chain is walked;
    /// a general body uses `ray_outside`. Only meaningful off the surface.
    pub(super) fn keeps_point(&self, point: Vec3, hits: &mut Vec<u32>, stack: &mut Vec<u32>) -> bool {
        if self.plane.is_none() {
            return self.ray_outside(point, hits, stack);
        }
        let mut node = self;
        loop {
            let Some(plane) = node.plane else { return true };
            let child = if plane.normal.dot(point) - plane.w > 0.0 { &node.front } else { &node.back };
            match child {
                Some(next) => node = next,
                // In front of a leaf plane is outside the body, behind it is in.
                None => return plane.normal.dot(point) - plane.w > 0.0,
            }
        }
    }

    /// Whether `point` is outside the surface, by counting crossings of a ray (odd means inside),
    /// using `clip_near`'s box hierarchy. A direction is abandoned if the ray meets a face edge-on,
    /// near its rim, or starts on one; `clip_near` asks exactly such rim points, hence skew directions.
    pub(super) fn ray_outside(&self, point: Vec3, hits: &mut Vec<u32>, stack: &mut Vec<u32>) -> bool {
        let Some(surface) = &self.surface else { return !self.inverted };
        let (lo, hi) = (surface.nodes[0].lo, surface.nodes[0].hi);
        // Beyond the body's extent there is nothing to count.
        let clear = |a: f64, b: f64, c: f64| a < b - EPSILON || a > c + EPSILON;
        if clear(point.x, lo.x, hi.x) || clear(point.y, lo.y, hi.y) || clear(point.z, lo.z, hi.z) {
            return !self.inverted;
        }
        let mut fallback = None;
        for dir in RAY_DIRECTIONS {
            surface.along_ray(point, dir, hits, stack);
            let mut crossings = 0usize;
            let mut clean = true;
            for &i in hits.iter() {
                let face = &self.faces[i as usize];
                let along = face.plane.normal.dot(dir);
                let distance = face.plane.normal.dot(point) - face.plane.w;
                if along.abs() < 1e-9 {
                    // Edge-on: crosses nothing, unless the point is in the face's plane, which is undecidable.
                    if distance.abs() < RAY_EPSILON {
                        clean = false;
                        break;
                    }
                    continue;
                }
                let t = -distance / along;
                if t < -RAY_EPSILON {
                    continue;
                }
                let margin = covered_by(face, point + dir * t);
                if margin < -RAY_EPSILON {
                    continue;
                }
                if margin < RAY_EPSILON || t < RAY_EPSILON {
                    // Through a face's rim, or starting on one.
                    clean = false;
                    break;
                }
                crossings += 1;
            }
            let outside = crossings.is_multiple_of(2) != self.inverted;
            if clean {
                return outside;
            }
            fallback.get_or_insert(outside);
        }
        fallback.unwrap_or(!self.inverted)
    }
}
