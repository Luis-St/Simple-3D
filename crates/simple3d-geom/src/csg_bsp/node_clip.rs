//! Clipping polygons against a tree, and reading the survivors back out.

use super::*;
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
    /// Clip `polygons` against this body, consuming them: a polygon that
    /// survives whole is handed straight back rather than copied.
    pub(super) fn clip_polygons(
        &self,
        polygons: Vec<Polygon>,
        scratch: &mut ClipScratch,
        give_up: crate::Abandon<'_>,
    ) -> Vec<Polygon> {
        if let (Some(convex), Some(surface)) = (&self.convex, &self.surface) {
            return clip_convex(convex, surface, polygons, scratch, give_up);
        }
        if let Some(surface) = &self.surface {
            return clip_near(self, surface, polygons, scratch, give_up);
        }
        let mut kept = Vec::new();
        // Pushed back-subtree first so the front one is popped first: the
        // surviving polygons come out in the same order the recursive walk
        // produced them, and so therefore does the mesh built from them.
        let mut stack: Vec<(&BspNode, Vec<Polygon>)> = vec![(self, polygons)];
        while let Some((node, polygons)) = stack.pop() {
            let Some(plane) = node.plane else {
                kept.extend(polygons);
                continue;
            };
            let mut cf = Vec::new();
            let mut cb = Vec::new();
            let mut front = Vec::new();
            let mut back = Vec::new();
            for p in polygons {
                // A polygon that comes near no face of this body at all cannot
                // be crossed by its surface, so it is wholly inside or wholly
                // outside, and one point settles which. The classic algorithm
                // splits it against the planes regardless, which is how a
                // 40 mm plate comes back from a union with a 20 mm sphere cut
                // to pieces along the sphere's *infinite* tangent planes -- out
                // at the plate's rim, a centimetre from the nearest part of the
                // sphere. The hierarchy is asked about the whole body rather
                // than this subtree on purpose: for a convex body the subtree is
                // a chain whose own box is the whole body, which proves nothing
                // about anything.
                let clear = match &self.surface {
                    Some(surface) => !surface.meets(p.bounds, &mut scratch.boxes),
                    None => false,
                };
                if clear {
                    if node.keeps_point(p.vertices[0], &mut scratch.ray, &mut scratch.ray_stack) {
                        kept.push(p);
                    }
                    continue;
                }
                scratch.splitter.split(&plane, p, &mut cf, &mut cb, &mut front, &mut back);
            }
            front.extend(cf);
            back.extend(cb);
            // Behind a leaf plane is solid, so what is left there is inside the
            // other body and does not survive the clip.
            if let Some(b) = &node.back {
                if !back.is_empty() {
                    stack.push((b, back));
                }
            }
            match &node.front {
                Some(f) if !front.is_empty() => stack.push((f, front)),
                Some(_) => {}
                None => kept.extend(front),
            }
        }
        kept
    }

    pub(super) fn clip_to(&mut self, other: &BspNode, scratch: &mut ClipScratch, give_up: crate::Abandon<'_>) {
        let mut stack: Vec<&mut BspNode> = vec![self];
        while let Some(node) = stack.pop() {
            if give_up() {
                node.polygons.clear();
                continue;
            }
            node.polygons = other.clip_polygons(std::mem::take(&mut node.polygons), scratch, give_up);
            stack.extend(node.front.as_deref_mut());
            stack.extend(node.back.as_deref_mut());
        }
    }

    pub(super) fn all_polygons(&self) -> Vec<Polygon> {
        let mut result = Vec::new();
        let mut stack: Vec<&BspNode> = vec![self];
        while let Some(node) = stack.pop() {
            result.extend(node.polygons.iter().cloned());
            // Back first, so the front subtree is popped -- and appended --
            // first, as in the recursive walk this replaces.
            stack.extend(node.back.as_deref());
            stack.extend(node.front.as_deref());
        }
        result
    }
}
