//! A bounding-box hierarchy over the polygons of one body: how it is built.

use super::*;
use crate::vec3::Vec3;

/// A bounding-volume hierarchy over the polygons of one body, answering two
/// questions the BSP tree itself answers badly.
///
/// *Does anything come near this box?* -- so a boolean can tell that a polygon
/// is nowhere near the other body's surface without walking that body's BSP,
/// which for a round primitive is a chain thousands of nodes long whose own
/// bounding box is the whole body and so proves nothing.
///
/// *Is every polygon behind this plane?* -- which is what says a plane splits
/// nothing, and lets `build` chain a convex body's faces in one pass instead of
/// re-classifying every remaining face at every one of its own planes.
pub(crate) struct BoxTree {
    pub(super) nodes: Vec<BoxNode>,
    /// Polygon indices, permuted so that each leaf owns a contiguous run.
    pub(super) order: Vec<u32>,
    pub(super) boxes: Vec<(Vec3, Vec3)>,
}

pub(crate) struct BoxNode {
    pub(super) lo: Vec3,
    pub(super) hi: Vec3,
    /// A split's two children, or a leaf's run of `order`.
    pub(super) kind: BoxKind,
}

pub(crate) enum BoxKind {
    Split(u32, u32),
    Leaf(u32, u32),
}

/// Below this many polygons a node stops dividing. Testing a handful directly
/// costs less than the branches to avoid them.
pub(crate) const BOX_LEAF: usize = 4;

impl BoxTree {
    pub(super) fn new(polygons: &[Polygon]) -> Option<BoxTree> {
        if polygons.is_empty() {
            return None;
        }
        let boxes: Vec<(Vec3, Vec3)> = polygons.iter().map(|p| p.bounds).collect();
        let order: Vec<u32> = (0..boxes.len() as u32).collect();
        let mut tree = BoxTree { nodes: Vec::new(), order, boxes };
        tree.split();
        Some(tree)
    }

    /// Divide at the median of the widest axis until each leaf is small.
    /// Iterative, like every other walk in this file: the input can be a
    /// hundred thousand faces.
    pub(super) fn split(&mut self) {
        let root = self.push_placeholder();
        let mut stack: Vec<(usize, usize, u32)> = vec![(0, self.order.len(), root)];
        while let Some((from, to, slot)) = stack.pop() {
            let (lo, hi) = self.enclosing(from, to);
            if to - from <= BOX_LEAF {
                self.nodes[slot as usize] = BoxNode { lo, hi, kind: BoxKind::Leaf(from as u32, to as u32) };
                continue;
            }
            let size = hi - lo;
            let axis = if size.x >= size.y && size.x >= size.z {
                0
            } else if size.y >= size.z {
                1
            } else {
                2
            };
            let boxes = &self.boxes;
            let key = |i: u32| {
                let b = boxes[i as usize];
                let c = (b.0 + b.1) / 2.0;
                match axis {
                    0 => c.x,
                    1 => c.y,
                    _ => c.z,
                }
            };
            let mid = from + (to - from) / 2;
            self.order[from..to].select_nth_unstable_by(mid - from, |&a, &b| key(a).total_cmp(&key(b)));
            let (left, right) = (self.push_placeholder(), self.push_placeholder());
            self.nodes[slot as usize] = BoxNode { lo, hi, kind: BoxKind::Split(left, right) };
            stack.push((from, mid, left));
            stack.push((mid, to, right));
        }
    }

    pub(super) fn enclosing(&self, from: usize, to: usize) -> (Vec3, Vec3) {
        let (mut lo, mut hi) = self.boxes[self.order[from] as usize];
        for &i in &self.order[from + 1..to] {
            let (blo, bhi) = self.boxes[i as usize];
            lo = lo.min(blo);
            hi = hi.max(bhi);
        }
        (lo, hi)
    }

    pub(super) fn push_placeholder(&mut self) -> u32 {
        self.nodes.push(BoxNode { lo: Vec3::ZERO, hi: Vec3::ZERO, kind: BoxKind::Leaf(0, 0) });
        (self.nodes.len() - 1) as u32
    }
}
