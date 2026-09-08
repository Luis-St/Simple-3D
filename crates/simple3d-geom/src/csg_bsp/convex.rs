//! A convex body as a set of half-spaces, for the cheap containment tests
//! that let whole subtrees be answered without splitting anything.

use super::*;
use crate::vec3::Vec3;

/// What a body whose own planes divide none of its faces -- a convex solid --
/// is, said directly rather than as a chain of BSP nodes.
///
/// The chain is a correct description of such a body and a ruinous one to clip
/// against: it holds one node per face plane, so a polygon is classified
/// against every plane of the body, and the planes are *infinite*. A 40 mm
/// plate meeting a 20 mm cap at 256 segments was split 103 million times to
/// produce 40,683 polygons, nearly all of those splits by a tangent plane whose
/// own face is millimetres away from where it cut.
///
/// The surface, though, is only ever near a polygon in a few places, and that
/// is enough to be exact: the body's boundary can only cross a polygon inside a
/// face, so cutting the polygon by the planes of the faces whose boxes come
/// near it leaves pieces that are each wholly inside the body or wholly
/// outside. `clip_convex` is that, and it is what turns the quadratic into a
/// pass over the faces that are actually there.
pub(crate) struct ConvexBody {
    /// The body's distinct face planes, in the order `chain` took them, always
    /// facing outwards -- `inverted` is carried separately rather than flipping
    /// them, so "in front of one of these" keeps its meaning of "outside".
    pub(super) planes: Vec<Plane>,
    /// Which of `planes` each polygon lies in, indexed as `surface` indexes the
    /// polygons the body was built from.
    pub(super) plane_of: Vec<u32>,
    /// The body's own box, so a point nowhere near it costs one test rather
    /// than a walk over every plane.
    pub(super) bounds: (Vec3, Vec3),
    /// Whether `invert` has made this the complement of the convex body. The
    /// planes and the hierarchy describe the same surface either way; only
    /// which side is solid changes.
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

    /// Is `point` inside the convex solid -- behind every one of its faces?
    ///
    /// Exact, and the only place the far planes are looked at. A point outside
    /// the body's box is settled by the box, and a point outside the body is
    /// settled by the first plane it is in front of; only a point genuinely
    /// inside pays for the whole list, and by then it has been reached by a
    /// piece the near faces could not decide.
    pub(super) fn contains(&self, point: Vec3) -> bool {
        if !boxes_meet((point, point), self.bounds) {
            return false;
        }
        self.planes.iter().all(|pl| pl.normal.dot(point) - pl.w <= 0.0)
    }
}
