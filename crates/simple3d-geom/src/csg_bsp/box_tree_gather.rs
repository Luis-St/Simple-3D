//! Reading polygons back out of the box hierarchy.

use super::*;
use crate::vec3::Vec3;
use std::ops::ControlFlow;

impl BoxTree {
    /// Every polygon whose box comes within `EPSILON` of `query`, by index: the faces whose planes
    /// can cut the query anywhere the surface is. `stack` is reused, as in `meets`.
    pub(super) fn gather(&self, query: (Vec3, Vec3), out: &mut Vec<u32>, stack: &mut Vec<u32>) {
        out.clear();
        let _ = self.visit(
            stack,
            |lo, hi| boxes_meet((lo, hi), query),
            |p| {
                if boxes_meet(self.boxes[p as usize], query) {
                    out.push(p);
                }
                ControlFlow::Continue(())
            },
        );
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
        let in_front = |p: u32| polygons[p as usize].vertices.iter().any(|v| plane.normal.dot(*v) - plane.w > EPSILON);
        self.visit(
            &mut Vec::new(),
            |lo, hi| !(furthest_corner(plane, lo, hi) <= EPSILON),
            |p| if in_front(p) { ControlFlow::Break(()) } else { ControlFlow::Continue(()) },
        )
        .is_continue()
    }
}
