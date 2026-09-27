//! The BSP node: its fields, the scratch space clipping borrows, and an iterative drop for deep trees.

use super::*;
use crate::vec3::Vec3;

pub(crate) struct BspNode {
    pub(super) plane: Option<Plane>,
    pub(super) front: Option<Box<BspNode>>,
    pub(super) back: Option<Box<BspNode>>,
    pub(super) polygons: Vec<Polygon>,
    /// A box hierarchy over the source polygons, held by the root, used by `clip_polygons` to prove a
    /// polygon meets no face. `None` only disables the shortcut.
    pub(super) surface: Option<BoxTree>,
    /// Each polygon's plane, indexed like `surface`; `clip_near` cuts by these alone.
    pub(super) face_planes: Vec<Plane>,
    /// The faces themselves, for bodies `clip_near` is used on, to check whether a coplanar piece is
    /// actually covered. Not kept for convex bodies, where `ConvexBody::contains` answers.
    pub(super) faces: Vec<Polygon>,
    /// Set when this body's planes divide none of its faces (`non_splitting_order`), so
    /// `clip_polygons` can skip the chain. Only on the root.
    pub(super) convex: Option<ConvexBody>,
    /// Which side of a general body's surface is solid, flipped by `invert` like
    /// `ConvexBody::inverted`, since parity alone cannot say.
    pub(super) inverted: bool,
}

/// Whether two boxes come within `EPSILON`: generous, since it must prove a polygon cannot meet a surface.
pub(crate) fn boxes_meet(a: (Vec3, Vec3), b: (Vec3, Vec3)) -> bool {
    let (alo, ahi) = a;
    let (blo, bhi) = b;
    alo.x <= bhi.x + EPSILON
        && blo.x <= ahi.x + EPSILON
        && alo.y <= bhi.y + EPSILON
        && blo.y <= ahi.y + EPSILON
        && alo.z <= bhi.z + EPSILON
        && blo.z <= ahi.z + EPSILON
}

/// Buffers a clip reuses between polygons, held for the whole boolean to avoid allocations.
#[derive(Default)]
pub(crate) struct ClipScratch {
    pub(super) splitter: Splitter,
    pub(super) boxes: Vec<u32>,
    /// The faces a convex clip found near one polygon, and their deduplicated planes.
    pub(super) near: Vec<u32>,
    pub(super) planes: Vec<u32>,
    /// Undecided pieces of a convex clip, and the next round.
    pub(super) pieces: Vec<Polygon>,
    pub(super) next: Vec<Polygon>,
    /// A general clip's undecided pieces.
    pub(super) work: Vec<Undecided>,
    /// The faces a parity ray could cross, and the walk that finds them.
    pub(super) ray: Vec<u32>,
    pub(super) ray_stack: Vec<u32>,
}

/// An unsettled piece of a general clip: the piece, the face index it is cut past, and any face it
/// lies in.
pub(crate) type Undecided = (Polygon, u32, Option<u32>);

impl Drop for BspNode {
    fn drop(&mut self) {
        // The compiler's drop glue recurses, overflowing on deep trees; unlink children into a list first.
        let mut stack: Vec<Box<BspNode>> = Vec::new();
        stack.extend(self.front.take());
        stack.extend(self.back.take());
        while let Some(mut node) = stack.pop() {
            stack.extend(node.front.take());
            stack.extend(node.back.take());
        }
    }
}
