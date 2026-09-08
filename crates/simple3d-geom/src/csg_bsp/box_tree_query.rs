//! Asking the box hierarchy what a plane or a ray runs into.

use super::*;
use crate::vec3::Vec3;

impl BoxTree {
    /// Does any polygon's box come within `EPSILON` of `query`?
    ///
    /// `stack` is the caller's, reused across calls: this is asked once per
    /// polygon per node of the tree being descended, and a fresh allocation
    /// for each was showing up as a share of the whole boolean.
    pub(super) fn meets(&self, query: (Vec3, Vec3), stack: &mut Vec<u32>) -> bool {
        stack.clear();
        stack.push(0);
        while let Some(i) = stack.pop() {
            let node = &self.nodes[i as usize];
            if !boxes_meet((node.lo, node.hi), query) {
                continue;
            }
            match node.kind {
                BoxKind::Split(left, right) => {
                    stack.push(left);
                    stack.push(right);
                }
                BoxKind::Leaf(from, to) => {
                    for &p in &self.order[from as usize..to as usize] {
                        if boxes_meet(self.boxes[p as usize], query) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Every polygon whose box comes within `EPSILON` of `query`, by index into
    /// the polygons the tree was built from.
    ///
    /// `meets` answers whether the list would be empty; this is what a convex
    /// clip needs instead, because the faces in it are the only ones whose
    /// planes can cut the query polygon anywhere the body's surface actually
    /// is. `stack` is the caller's, for the same reason as in `meets`.
    /// Every polygon whose box the ray from `origin` along `dir` could enter,
    /// in no particular order. The slab test, with the divisions done once per
    /// query rather than once per node.
    ///
    /// An infinite ray rather than a segment: a parity count has to see every
    /// face ahead of the point, however far away.
    pub(super) fn along_ray(&self, origin: Vec3, dir: Vec3, out: &mut Vec<u32>, stack: &mut Vec<u32>) {
        let inv = Vec3::new(1.0 / dir.x, 1.0 / dir.y, 1.0 / dir.z);
        // Generous by `EPSILON`, like every other box test here: the point of
        // one is to prove a face *cannot* be crossed, and a box the ray only
        // just misses proves nothing.
        let hits = |lo: Vec3, hi: Vec3| -> bool {
            let mut near = f64::NEG_INFINITY;
            let mut far = f64::INFINITY;
            for (o, d, l, h) in [
                (origin.x, inv.x, lo.x - EPSILON, hi.x + EPSILON),
                (origin.y, inv.y, lo.y - EPSILON, hi.y + EPSILON),
                (origin.z, inv.z, lo.z - EPSILON, hi.z + EPSILON),
            ] {
                let (mut t0, mut t1) = ((l - o) * d, (h - o) * d);
                if t0 > t1 {
                    std::mem::swap(&mut t0, &mut t1);
                }
                // A direction with a zero component gives infinities here, and
                // they compare the right way round: the slab is missed entirely
                // only when the origin is outside it, which makes both bounds
                // the same infinity.
                near = near.max(t0);
                far = far.min(t1);
            }
            far >= near.max(0.0)
        };
        out.clear();
        stack.clear();
        stack.push(0);
        while let Some(i) = stack.pop() {
            let node = &self.nodes[i as usize];
            if !hits(node.lo, node.hi) {
                continue;
            }
            match node.kind {
                BoxKind::Split(left, right) => {
                    stack.push(left);
                    stack.push(right);
                }
                BoxKind::Leaf(from, to) => {
                    for &p in &self.order[from as usize..to as usize] {
                        let (lo, hi) = self.boxes[p as usize];
                        if hits(lo, hi) {
                            out.push(p);
                        }
                    }
                }
            }
        }
    }
}

/// How far in front of `plane` the furthest corner of the box lies.
pub(crate) fn furthest_corner(plane: &Plane, lo: Vec3, hi: Vec3) -> f64 {
    let n = plane.normal;
    let pick = |a: f64, l: f64, h: f64| if a >= 0.0 { h } else { l };
    let far = Vec3::new(pick(n.x, lo.x, hi.x), pick(n.y, lo.y, hi.y), pick(n.z, lo.z, hi.z));
    n.dot(far) - plane.w
}
