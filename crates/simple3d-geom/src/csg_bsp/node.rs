//! The BSP node itself: its fields, the scratch space its clipping borrows,
//! and the iterative drop that keeps a deep tree off the stack.

use super::*;
use crate::vec3::Vec3;

pub(crate) struct BspNode {
    pub(super) plane: Option<Plane>,
    pub(super) front: Option<Box<BspNode>>,
    pub(super) back: Option<Box<BspNode>>,
    pub(super) polygons: Vec<Polygon>,
    /// A hierarchy over the boxes of the polygons this tree was built from,
    /// held by the root and used by `clip_polygons` to prove that a polygon
    /// meets no face of this body at all. `None` disables that shortcut, which
    /// only ever costs time.
    pub(super) surface: Option<BoxTree>,
    /// The plane of each of those polygons, indexed as `surface` indexes them.
    /// `clip_near` cuts by these and by nothing else, so a body's far faces
    /// never reach a polygon their own surface is nowhere near.
    pub(super) face_planes: Vec<Plane>,
    /// The faces themselves, kept only for a body `clip_near` will be used on:
    /// deciding a piece that lies in a face's plane needs to know whether the
    /// face actually covers it, and a plane cannot say. A convex body needs
    /// none of this -- its face planes support it, so anything lying in one and
    /// outside the face is outside the body, which `ConvexBody::contains`
    /// already answers -- so the copy is not made there.
    pub(super) faces: Vec<Polygon>,
    /// Set when this body's own planes divide none of its faces, which is what
    /// `non_splitting_order` proves. `clip_polygons` uses it instead of
    /// descending the chain. Only a root carries one; the nodes `chain` and
    /// `chain` create are empty.
    pub(super) convex: Option<ConvexBody>,
    /// Which side of a general body's surface is solid. Flipped by `invert`,
    /// the way `ConvexBody::inverted` is: a general body has no tree to turn
    /// inside out any more, only its faces and the parity test over them, and
    /// parity says nothing about which side of the count is the solid one.
    pub(super) inverted: bool,
}

/// Do the two boxes come within `EPSILON` of each other? Deliberately generous:
/// the point of the test is to prove that a polygon *cannot* meet a surface, and
/// a box that only just misses proves nothing.
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

/// The buffers a clip reuses from one polygon to the next. Held by `op` for
/// the whole boolean rather than by any one call, so the deepest chain costs
/// no more allocations than the shallowest tree.
#[derive(Default)]
pub(crate) struct ClipScratch {
    pub(super) splitter: Splitter,
    pub(super) boxes: Vec<u32>,
    /// The faces a convex clip found near one polygon, and the planes they lie
    /// in, deduplicated.
    pub(super) near: Vec<u32>,
    pub(super) planes: Vec<u32>,
    /// The pieces a convex clip has not yet decided, and the next round of them.
    pub(super) pieces: Vec<Polygon>,
    pub(super) next: Vec<Polygon>,
    /// A general clip's undecided pieces.
    pub(super) work: Vec<Undecided>,
    /// The faces a parity ray could cross, and the hierarchy walk that finds
    /// them.
    pub(super) ray: Vec<u32>,
    pub(super) ray_stack: Vec<u32>,
}

/// A piece a general clip has not settled yet: the piece itself, the face index
/// it has been cut past, and whichever face it was found to lie in the plane of.
pub(crate) type Undecided = (Polygon, u32, Option<u32>);

impl Drop for BspNode {
    fn drop(&mut self) {
        // The compiler's own drop glue is recursive, so a deep tree overflows
        // the stack on the way out just as surely as on the way in. Unlink the
        // children into a list first; each box then drops with no children of
        // its own left to recurse into.
        let mut stack: Vec<Box<BspNode>> = Vec::new();
        stack.extend(self.front.take());
        stack.extend(self.back.take());
        while let Some(mut node) = stack.pop() {
            stack.extend(node.front.take());
            stack.extend(node.back.take());
        }
    }
}
