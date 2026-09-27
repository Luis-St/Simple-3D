//! A convex face carried through the tree, and how it sits against a plane.

use super::*;
use crate::vec3::Vec3;

#[derive(Clone, Debug)]
pub(crate) struct Polygon {
    pub(super) vertices: Vec<Vec3>,
    pub(super) plane: Plane,
    /// The source body, carried through every clip and split so surviving faces keep it.
    pub(super) tag: u32,
    /// Cached bounding box: `clip_polygons` asks for it per polygon per node, and recomputing it walked
    /// every vertex tens of thousands of times over.
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

    /// A point strictly inside the (convex) polygon. A vertex would not do: a clipped piece's vertex
    /// lies on the cutting plane, where a side test has no answer.
    pub(super) fn centroid(&self) -> Vec3 {
        let mut sum = Vec3::ZERO;
        for v in &self.vertices {
            sum = sum + *v;
        }
        sum / self.vertices.len() as f64
    }

    /// Reverses winding and plane; the box is unchanged.
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
