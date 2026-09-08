//! A convex face carried through the tree, and how it sits against a plane.

use super::*;
use crate::vec3::Vec3;

#[derive(Clone, Debug)]
pub(crate) struct Polygon {
    pub(super) vertices: Vec<Vec3>,
    pub(super) plane: Plane,
    /// Which body this face came from, carried unchanged through every clip
    /// and split so the surfaces that survive a boolean still say what they
    /// belonged to.
    pub(super) tag: u32,
    /// The polygon's own bounding box, carried beside it rather than
    /// recomputed. `clip_polygons` asks for it once per polygon per node of
    /// the tree it is descending, and for a convex operand that tree is a
    /// chain one node deep per face -- so recomputing it there walked every
    /// vertex of every fragment tens of thousands of times over.
    pub(super) bounds: (Vec3, Vec3),
}

impl Polygon {
    pub(super) fn new(vertices: Vec<Vec3>, plane: Plane, tag: u32) -> Polygon {
        let mut lo = vertices[0];
        let mut hi = vertices[0];
        for v in &vertices[1..] {
            lo = lo.min(*v);
            hi = hi.max(*v);
        }
        Polygon { vertices, plane, tag, bounds: (lo, hi) }
    }

    /// A point strictly inside the polygon: every polygon here is convex, so
    /// the average of its vertices is interior to it however thin it is. A
    /// vertex would not do -- a vertex of a clipped piece lies *on* the plane
    /// that cut it, which is exactly where a side test has no answer.
    pub(super) fn centroid(&self) -> Vec3 {
        let mut sum = Vec3::ZERO;
        for v in &self.vertices {
            sum = sum + *v;
        }
        sum / self.vertices.len() as f64
    }

    /// Flipping reverses the winding and the plane; the box is the same box.
    pub(super) fn flip(&self) -> Polygon {
        let mut v = self.vertices.clone();
        v.reverse();
        Polygon { vertices: v, plane: self.plane.flip(), tag: self.tag, bounds: self.bounds }
    }
}

pub(crate) const COPLANAR: i32 = 0;

pub(crate) const FRONT: i32 = 1;

pub(crate) const BACK: i32 = 2;

pub(crate) const SPANNING: i32 = 3;
