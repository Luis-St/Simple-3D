//! Reading polygons back out of the box hierarchy.

use super::*;
use crate::vec3::Vec3;

impl BoxTree {
    pub(super) fn gather(&self, query: (Vec3, Vec3), out: &mut Vec<u32>, stack: &mut Vec<u32>) {
        out.clear();
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
                            out.push(p);
                        }
                    }
                }
            }
        }
    }

    /// Is every polygon in `polygons` on or behind `plane`?
    ///
    /// A box whose every corner is behind the plane settles a whole subtree at
    /// once, which is what keeps this cheap: for a sphere and one of its own
    /// tangent planes, all but the faces around the point of tangency are
    /// pruned in a handful of steps. Only the leaves that survive are tested
    /// vertex by vertex, and those are exact -- a box corner can poke through a
    /// tilted plane that no vertex reaches.
    pub(super) fn all_behind(&self, plane: &Plane, polygons: &[Polygon]) -> bool {
        let mut stack = vec![0u32];
        while let Some(i) = stack.pop() {
            let node = &self.nodes[i as usize];
            if furthest_corner(plane, node.lo, node.hi) <= EPSILON {
                continue;
            }
            match node.kind {
                BoxKind::Split(left, right) => {
                    stack.push(left);
                    stack.push(right);
                }
                BoxKind::Leaf(from, to) => {
                    for &p in &self.order[from as usize..to as usize] {
                        for v in &polygons[p as usize].vertices {
                            if plane.normal.dot(*v) - plane.w > EPSILON {
                                return false;
                            }
                        }
                    }
                }
            }
        }
        true
    }
}
