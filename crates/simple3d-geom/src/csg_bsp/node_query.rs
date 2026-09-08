//! Point and ray classification against a built tree.

use super::*;
use crate::vec3::Vec3;

/// Every walk over the tree below is written with an explicit stack rather
/// than by recursion, and the tree drops itself the same way.
///
/// This is not a matter of taste. The splitting plane is the first polygon's
/// own plane -- the classic auto-partition -- and a *convex* body defeats it
/// completely: every face of a sphere has the whole sphere behind it, so no
/// face ever divides the rest and the "tree" comes out as a chain one node per
/// polygon deep. A sphere or a spherical cap at 128 segments is some eight
/// thousand faces, and eight thousand nested calls of `build` (or of `clip_to`,
/// or of the compiler's own drop glue for the chain of boxes) is past the end of
/// the thread's stack: the process died on the spot, taking the user's unsaved
/// document with it, the moment the segment count of a round primitive touching
/// another body was raised. Depth now costs heap, which the machine has.
///
/// The chain is still a chain: that is what a convex body's own face planes
/// make, and clipping down one is cheap because a polygon leaves it at the
/// first plane it is in front of. Building one is what used to be expensive,
/// and `non_splitting_order` is how that stopped being so.
impl BspNode {
    /// Is `point` outside this body -- and so kept by a clip against it?
    ///
    /// A convex body is a chain of its own face planes and is walked: front at a
    /// leaf is outside it, back at a leaf is inside. A general body has no tree
    /// to walk any more (see `BspNode::new`) and is answered by `ray_outside`.
    ///
    /// Only meaningful for a point the surface does not pass through, which is
    /// exactly where it is used.
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

    /// Is `point` outside the surface, by counting the faces a ray from it
    /// crosses? An odd count means it started inside.
    ///
    /// The faces come from the same box hierarchy `clip_near` gathers from, so
    /// a query costs a walk of that hierarchy rather than a pass over every
    /// triangle of the body.
    ///
    /// A direction is abandoned the moment the ray meets a face edge-on, or
    /// crosses one within a hair of its rim, or starts on one: a count taken
    /// along an edge or through a vertex is the classic way a parity test comes
    /// out wrong. That is not a rare case here -- `clip_near` asks this exactly
    /// about the pieces that lie *in* the plane of one of the body's faces but
    /// off the end of it, and such a point sits on the rim of every face
    /// perpendicular to that one. The six axis directions are all unusable
    /// there, which is why the directions tried are deliberately skew: no plane
    /// of an axis-aligned body is parallel to one, and no edge of one lies
    /// along one.
    pub(super) fn ray_outside(&self, point: Vec3, hits: &mut Vec<u32>, stack: &mut Vec<u32>) -> bool {
        let Some(surface) = &self.surface else { return !self.inverted };
        let (lo, hi) = (surface.nodes[0].lo, surface.nodes[0].hi);
        // Beyond the body's own extent there is nothing to count.
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
                    // Edge-on to the ray: it crosses nothing, unless the point
                    // is in the face's plane, where the answer is undecidable.
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
                    // Through the rim of a face, or starting on one.
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
