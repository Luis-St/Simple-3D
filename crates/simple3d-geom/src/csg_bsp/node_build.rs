//! Building a BSP tree from polygons, and turning one inside out.

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
                    // No tree. A general body used to be partitioned into a BSP
                    // of its own planes, and that tree was the body: it held the
                    // polygons a clip handed back, and it answered "is this
                    // point inside". `clip_near` took the first of those jobs
                    // over -- it cuts by the planes of the faces that come near
                    // a piece, and never by the rest -- and the second is now
                    // `ray_outside`, which counts the faces a ray from the point
                    // crosses. Neither needs the polygons partitioned, and
                    // partitioning them was ruinous: a 200 mm landscape's 18,811
                    // faces came out of its own tree as 102,801 fragments, two
                    // seconds of work, and it was the *fragments* a union
                    // emitted -- a surface riddled with T-junctions and slivers
                    // a hundred millimetres from anything the other body
                    // touched, which `repair` could not always close. Half the
                    // shipped landscapes reported `Union produced non-manifold
                    // geometry` the moment any shape overlapped them. The faces
                    // now come out of a union as the faces that went in.
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
        // The planes a convex body is described by face outwards whichever way
        // round the solid is; inverting swaps which side of them is solid.
        if let Some(convex) = &mut self.convex {
            convex.inverted = !convex.inverted;
        }
        // The stored face planes are what `clip_near` cuts by, and which side
        // of one of them is solid is the whole of the coplanar rule; a body
        // turned inside out has to turn them with it.
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

    /// Build a set that no plane of its own divides, as the chain of nodes the
    /// general path would have arrived at -- without the work of arriving.
    ///
    /// `groups` lists the polygons of each plane, in the order the planes are
    /// to be chained. Each node keeps the polygons lying in its own plane, just
    /// as a general BSP build would put coplanar polygons at the node; everything
    /// further down is behind it.
    pub(super) fn chain(&mut self, polygons: Vec<Polygon>, groups: Vec<Vec<usize>>) {
        let mut polygons: Vec<Option<Polygon>> = polygons.into_iter().map(Some).collect();
        let mut node = self;
        let last = groups.len() - 1;
        for (i, group) in groups.into_iter().enumerate() {
            node.plane = Some(polygons[group[0]].as_ref().expect("each polygon is in one group").plane);
            node.polygons = group.iter().map(|&i| polygons[i].take().expect("groups do not overlap")).collect();
            // No node past the last plane. A node with no plane of its own
            // *keeps* everything that reaches it, so a trailing empty one would
            // hand back the far side of the body as though it were outside --
            // the deepest back child being absent is what says "solid here".
            if i < last {
                node = node.back.get_or_insert_with(|| Box::new(BspNode::new(Vec::new())));
            }
        }
    }
}
