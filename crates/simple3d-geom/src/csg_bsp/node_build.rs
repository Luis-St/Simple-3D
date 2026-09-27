//! Building a BSP tree from polygons, and turning one inside out.

use super::*;
/// Every tree walk uses an explicit stack, and the tree drops itself iteratively.
///
/// A convex body defeats auto-partitioning, so its tree is a chain one node per face; a
/// 128-segment sphere's thousands of nested calls overflowed the stack and crashed the
/// application. Clipping down such a chain is cheap; building it is handled by
/// `non_splitting_order`.
impl BspNode {
    pub(super) fn new(polygons: Vec<Polygon>) -> BspNode {
        let mut node = BspNode {
            plane: None,
            front: None,
            back: None,
            polygons: Vec::new(),
            surface: None,
            face_planes: Vec::new(),
            faces: Vec::new(),
            convex: None,
            inverted: false,
        };
        if !polygons.is_empty() {
            let surface = BoxTree::new(&polygons);
            let face_planes: Vec<Plane> = polygons.iter().map(|p| p.plane).collect();
            let convex_order = non_splitting_order(&polygons);
            let faces = if convex_order.is_some() { Vec::new() } else { polygons.clone() };
            match convex_order {
                Some(groups) => {
                    node.convex = Some(ConvexBody::new(&polygons, &groups));
                    node.chain(polygons, groups);
                }
                None => {
                    // No tree: `clip_near` cuts by nearby face planes and `ray_outside` answers inside tests, so the
                    // polygons need no partitioning. Partitioning a large landscape by its own planes produced
                    // fragments that unions then emitted, often non-manifold; faces now leave a union as they entered.
                    node.polygons = polygons;
                }
            }
            node.surface = surface;
            node.face_planes = face_planes;
            node.faces = faces;
        }
        node
    }

    pub(super) fn invert(&mut self) {
        // A convex body's outward planes stay; inverting swaps which side is solid.
        if let Some(convex) = &mut self.convex {
            convex.inverted = !convex.inverted;
        }
        // The face planes `clip_near` cuts by decide the coplanar rule, so they turn with the body.
        for plane in self.face_planes.iter_mut() {
            *plane = plane.flip();
        }
        for face in self.faces.iter_mut() {
            *face = face.flip();
        }
        let mut stack: Vec<&mut BspNode> = vec![self];
        while let Some(node) = stack.pop() {
            node.inverted = !node.inverted;
            for p in node.polygons.iter_mut() {
                *p = p.flip();
            }
            if let Some(plane) = &mut node.plane {
                *plane = plane.flip();
            }
            std::mem::swap(&mut node.front, &mut node.back);
            stack.extend(node.front.as_deref_mut());
            stack.extend(node.back.as_deref_mut());
        }
    }

    /// Build a set no plane of its own divides directly as the chain the general build would reach.
    /// `groups` lists each plane's polygons in chain order; each node keeps its coplanar polygons.
    pub(super) fn chain(&mut self, polygons: Vec<Polygon>, groups: Vec<Vec<usize>>) {
        let mut polygons: Vec<Option<Polygon>> = polygons.into_iter().map(Some).collect();
        let mut node = self;
        let last = groups.len() - 1;
        for (i, group) in groups.into_iter().enumerate() {
            node.plane = Some(polygons[group[0]].as_ref().expect("each polygon is in one group").plane);
            node.polygons = group.iter().map(|&i| polygons[i].take().expect("groups do not overlap")).collect();
            // No node past the last plane: an empty node would keep everything reaching it, whereas a missing
            // deepest back child is what means "solid here".
            if i < last {
                node = node.back.get_or_insert_with(|| Box::new(BspNode::new(Vec::new())));
            }
        }
    }
}
