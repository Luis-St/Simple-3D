//! Clipping polygons against a tree, and reading the survivors back out.

use super::*;
/// Tree walks use an explicit stack, since a convex body's tree is a deep chain (see `node_build`).
impl BspNode {
    /// Clip `polygons` against this body, consuming them; whole survivors are returned uncopied.
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
        // Back subtree pushed first so the front pops first, keeping the recursive walk's output order.
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
                // A polygon near no face of the body is wholly in or out, settled by one point, instead of being
                // split by far-off infinite planes. The whole body's hierarchy is asked, since a convex chain's
                // subtree box proves nothing.
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
            // Behind a leaf plane is solid, so what is left there does not survive.
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
            // Back first, so the front subtree pops and appends first, as in the recursive walk.
            stack.extend(node.back.as_deref());
            stack.extend(node.front.as_deref());
        }
        result
    }
}
