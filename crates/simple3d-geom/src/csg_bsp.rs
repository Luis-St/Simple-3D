//! Self-contained BSP-tree boolean CSG kernel (union / subtract / intersect),
//! following the classic algorithm popularised by Evan Wallace's csg.js and
//! used by many browser-based CAD tools. We don't depend on an external CSG
//! crate: at the time this was written every published version of the one
//! obvious crate (`csgrs`) pulls in a yanked transitive dependency and fails
//! to build from crates.io, and pulling in a big C++ kernel would break the
//! "fully self-contained, nothing to install" constraint. This kernel is
//! deliberately small and easy to audit instead.
//!
//! Known limitation: plane classification uses a fixed epsilon rather than
//! exact/rational arithmetic, so pathologically thin or near-degenerate
//! inputs can in principle still produce a non-manifold result. `Mesh::manifold_issue`
//! is used by the evaluator to detect that and fail loudly on the offending
//! node rather than emit broken geometry, per the spec's requirement.

use crate::mesh::Mesh;
use crate::vec3::Vec3;

const EPSILON: f64 = 1e-5;

#[derive(Clone, Copy, Debug)]
struct Plane {
    normal: Vec3,
    w: f64,
}

impl Plane {
    fn from_points(a: Vec3, b: Vec3, c: Vec3) -> Option<Plane> {
        let n = (b - a).cross(c - a);
        if n.length() < 1e-12 {
            return None;
        }
        let n = n.normalized();
        Some(Plane { normal: n, w: n.dot(a) })
    }

    fn flip(&self) -> Plane {
        Plane { normal: -self.normal, w: -self.w }
    }
}

#[derive(Clone, Debug)]
struct Polygon {
    vertices: Vec<Vec3>,
    plane: Plane,
    /// Which body this face came from, carried unchanged through every clip
    /// and split so the surfaces that survive a boolean still say what they
    /// belonged to.
    tag: u32,
}

impl Polygon {
    fn flip(&self) -> Polygon {
        let mut v = self.vertices.clone();
        v.reverse();
        Polygon { vertices: v, plane: self.plane.flip(), tag: self.tag }
    }
}

const COPLANAR: i32 = 0;
const FRONT: i32 = 1;
const BACK: i32 = 2;
const SPANNING: i32 = 3;

/// Split `poly` by `plane`, appending results into the four buckets.
fn split_polygon(
    plane: &Plane,
    poly: &Polygon,
    coplanar_front: &mut Vec<Polygon>,
    coplanar_back: &mut Vec<Polygon>,
    front: &mut Vec<Polygon>,
    back: &mut Vec<Polygon>,
) {
    let mut polygon_type = 0;
    let mut types = Vec::with_capacity(poly.vertices.len());
    for v in &poly.vertices {
        let t = plane.normal.dot(*v) - plane.w;
        let ty = if t < -EPSILON {
            BACK
        } else if t > EPSILON {
            FRONT
        } else {
            COPLANAR
        };
        polygon_type |= ty;
        types.push(ty);
    }

    match polygon_type {
        COPLANAR => {
            if plane.normal.dot(poly.plane.normal) > 0.0 {
                coplanar_front.push(poly.clone());
            } else {
                coplanar_back.push(poly.clone());
            }
        }
        FRONT => front.push(poly.clone()),
        BACK => back.push(poly.clone()),
        _ => {
            let mut f: Vec<Vec3> = Vec::new();
            let mut b: Vec<Vec3> = Vec::new();
            let n = poly.vertices.len();
            for i in 0..n {
                let j = (i + 1) % n;
                let (ti, tj) = (types[i], types[j]);
                let (vi, vj) = (poly.vertices[i], poly.vertices[j]);
                if ti != BACK {
                    f.push(vi);
                }
                if ti != FRONT {
                    b.push(vi);
                }
                if (ti | tj) == SPANNING {
                    let denom = plane.normal.dot(vj - vi);
                    let t = (plane.w - plane.normal.dot(vi)) / denom;
                    let v = vi.lerp(vj, t);
                    f.push(v);
                    b.push(v);
                }
            }
            if f.len() >= 3 {
                front.push(Polygon { vertices: f, plane: poly.plane, tag: poly.tag });
            }
            if b.len() >= 3 {
                back.push(Polygon { vertices: b, plane: poly.plane, tag: poly.tag });
            }
        }
    }
}

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
struct BoxTree {
    nodes: Vec<BoxNode>,
    /// Polygon indices, permuted so that each leaf owns a contiguous run.
    order: Vec<u32>,
    boxes: Vec<(Vec3, Vec3)>,
}

struct BoxNode {
    lo: Vec3,
    hi: Vec3,
    /// A split's two children, or a leaf's run of `order`.
    kind: BoxKind,
}

enum BoxKind {
    Split(u32, u32),
    Leaf(u32, u32),
}

/// Below this many polygons a node stops dividing. Testing a handful directly
/// costs less than the branches to avoid them.
const BOX_LEAF: usize = 4;

impl BoxTree {
    fn new(polygons: &[Polygon]) -> Option<BoxTree> {
        let boxes: Vec<(Vec3, Vec3)> =
            polygons.iter().filter_map(|p| polygon_bounds(std::slice::from_ref(p))).collect();
        if boxes.len() != polygons.len() || boxes.is_empty() {
            return None;
        }
        let order: Vec<u32> = (0..boxes.len() as u32).collect();
        let mut tree = BoxTree { nodes: Vec::new(), order, boxes };
        tree.split();
        Some(tree)
    }

    /// Divide at the median of the widest axis until each leaf is small.
    /// Iterative, like every other walk in this file: the input can be a
    /// hundred thousand faces.
    fn split(&mut self) {
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

    fn enclosing(&self, from: usize, to: usize) -> (Vec3, Vec3) {
        let (mut lo, mut hi) = self.boxes[self.order[from] as usize];
        for &i in &self.order[from + 1..to] {
            let (blo, bhi) = self.boxes[i as usize];
            lo = lo.min(blo);
            hi = hi.max(bhi);
        }
        (lo, hi)
    }

    fn push_placeholder(&mut self) -> u32 {
        self.nodes.push(BoxNode { lo: Vec3::ZERO, hi: Vec3::ZERO, kind: BoxKind::Leaf(0, 0) });
        (self.nodes.len() - 1) as u32
    }

    /// Does any polygon's box come within `EPSILON` of `query`?
    fn meets(&self, query: (Vec3, Vec3)) -> bool {
        let mut stack = vec![0u32];
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

    /// Is every polygon in `polygons` on or behind `plane`?
    ///
    /// A box whose every corner is behind the plane settles a whole subtree at
    /// once, which is what keeps this cheap: for a sphere and one of its own
    /// tangent planes, all but the faces around the point of tangency are
    /// pruned in a handful of steps. Only the leaves that survive are tested
    /// vertex by vertex, and those are exact -- a box corner can poke through a
    /// tilted plane that no vertex reaches.
    fn all_behind(&self, plane: &Plane, polygons: &[Polygon]) -> bool {
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

/// How far in front of `plane` the furthest corner of the box lies.
fn furthest_corner(plane: &Plane, lo: Vec3, hi: Vec3) -> f64 {
    let n = plane.normal;
    let pick = |a: f64, l: f64, h: f64| if a >= 0.0 { h } else { l };
    let far = Vec3::new(pick(n.x, lo.x, hi.x), pick(n.y, lo.y, hi.y), pick(n.z, lo.z, hi.z));
    n.dot(far) - plane.w
}

/// A plane's identity, sign-independent: two faces of a solid that lie in the
/// same plane belong at the same BSP node whichever way they face.
fn unoriented_plane_key(p: &Plane) -> (i64, i64, i64, i64) {
    let flip = if p.normal.x.abs() > 1e-9 {
        p.normal.x < 0.0
    } else if p.normal.y.abs() > 1e-9 {
        p.normal.y < 0.0
    } else {
        p.normal.z < 0.0
    };
    let p = if flip { p.flip() } else { *p };
    plane_key(&p)
}

struct BspNode {
    plane: Option<Plane>,
    front: Option<Box<BspNode>>,
    back: Option<Box<BspNode>>,
    polygons: Vec<Polygon>,
    /// A hierarchy over the boxes of the polygons this tree was built from,
    /// held by the root and used by `clip_polygons` to prove that a polygon
    /// meets no face of this body at all. `None` disables that shortcut, which
    /// only ever costs time.
    surface: Option<BoxTree>,
}

fn polygon_bounds(polygons: &[Polygon]) -> Option<(Vec3, Vec3)> {
    let mut bounds: Option<(Vec3, Vec3)> = None;
    for p in polygons {
        for v in &p.vertices {
            bounds = Some(match bounds {
                None => (*v, *v),
                Some((lo, hi)) => (lo.min(*v), hi.max(*v)),
            });
        }
    }
    bounds
}

/// Do the two boxes come within `EPSILON` of each other? Deliberately generous:
/// the point of the test is to prove that a polygon *cannot* meet a surface, and
/// a box that only just misses proves nothing.
fn boxes_meet(a: (Vec3, Vec3), b: (Vec3, Vec3)) -> bool {
    let (alo, ahi) = a;
    let (blo, bhi) = b;
    alo.x <= bhi.x + EPSILON
        && blo.x <= ahi.x + EPSILON
        && alo.y <= bhi.y + EPSILON
        && blo.y <= ahi.y + EPSILON
        && alo.z <= bhi.z + EPSILON
        && blo.z <= ahi.z + EPSILON
}

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
    fn new(polygons: Vec<Polygon>) -> BspNode {
        let mut node = BspNode { plane: None, front: None, back: None, polygons: Vec::new(), surface: None };
        if !polygons.is_empty() {
            let surface = BoxTree::new(&polygons);
            node.build(polygons);
            node.surface = surface;
        }
        node
    }

    fn invert(&mut self) {
        let mut stack: Vec<&mut BspNode> = vec![self];
        while let Some(node) = stack.pop() {
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

    /// Is `point` on the solid side of this subtree?
    ///
    /// The same walk `clip_polygons` makes, for a single point and without
    /// splitting anything: front at a leaf is outside the body, back at a leaf
    /// is inside it. Only meaningful for a point the surface does not pass
    /// near, which is exactly where it is used.
    fn keeps_point(&self, point: Vec3) -> bool {
        let mut node = self;
        loop {
            let Some(plane) = node.plane else { return true };
            let child = if plane.normal.dot(point) - plane.w > 0.0 { &node.front } else { &node.back };
            match child {
                Some(next) => node = next,
                // In front of a leaf plane is outside the body, behind it is in.
                None => return plane.normal.dot(point) - plane.w > 0.0,
            }
        }
    }

    fn clip_polygons(&self, polygons: &[Polygon]) -> Vec<Polygon> {
        let mut kept = Vec::new();
        // Pushed back-subtree first so the front one is popped first: the
        // surviving polygons come out in the same order the recursive walk
        // produced them, and so therefore does the mesh built from them.
        let mut stack: Vec<(&BspNode, Vec<Polygon>)> = vec![(self, polygons.to_vec())];
        while let Some((node, polygons)) = stack.pop() {
            let Some(plane) = node.plane else {
                kept.extend(polygons);
                continue;
            };
            let mut cf = Vec::new();
            let mut cb = Vec::new();
            let mut front = Vec::new();
            let mut back = Vec::new();
            for p in &polygons {
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
                let clear = match (&self.surface, polygon_bounds(std::slice::from_ref(p))) {
                    (Some(surface), Some(poly)) => !surface.meets(poly),
                    _ => false,
                };
                if clear {
                    if node.keeps_point(p.vertices[0]) {
                        kept.push(p.clone());
                    }
                    continue;
                }
                split_polygon(&plane, p, &mut cf, &mut cb, &mut front, &mut back);
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

    fn clip_to(&mut self, other: &BspNode) {
        let mut stack: Vec<&mut BspNode> = vec![self];
        while let Some(node) = stack.pop() {
            node.polygons = other.clip_polygons(&node.polygons);
            stack.extend(node.front.as_deref_mut());
            stack.extend(node.back.as_deref_mut());
        }
    }

    fn all_polygons(&self) -> Vec<Polygon> {
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

    fn build(&mut self, polygons: Vec<Polygon>) {
        // The hierarchy describes the polygons the tree already had; polygons
        // arriving now are not in it, so the shortcut it serves is withdrawn.
        // Nothing in `op` clips a tree after building into it.
        self.surface = None;
        let mut stack: Vec<(&mut BspNode, Vec<Polygon>)> = vec![(self, polygons)];
        while let Some((node, polygons)) = stack.pop() {
            if polygons.is_empty() {
                continue;
            }
            if node.plane.is_none() {
                if let Some(groups) = non_splitting_order(&polygons) {
                    node.chain(polygons, groups);
                    continue;
                }
            }
            node.build_one_level(polygons, &mut stack);
        }
    }

    /// Build a set that no plane of its own divides, as the chain of nodes the
    /// general path would have arrived at -- without the work of arriving.
    ///
    /// `groups` lists the polygons of each plane, in the order the planes are
    /// to be chained. Each node keeps the polygons lying in its own plane, just
    /// as `build_one_level` puts coplanar polygons at the node; everything
    /// further down is behind it.
    fn chain(&mut self, polygons: Vec<Polygon>, groups: Vec<Vec<usize>>) {
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

    /// Partition `polygons` by this node's plane, keeping what is coplanar with
    /// it and handing the two sides to the children.
    fn build_one_level<'a>(&'a mut self, polygons: Vec<Polygon>, stack: &mut Vec<(&'a mut BspNode, Vec<Polygon>)>) {
        let node = self;
        if node.plane.is_none() {
            node.plane = Some(polygons[0].plane);
        }
        let plane = node.plane.unwrap();
        let mut front = Vec::new();
        let mut back = Vec::new();
        for p in polygons {
            let mut cf = Vec::new();
            let mut cb = Vec::new();
            let mut fr = Vec::new();
            let mut bk = Vec::new();
            split_polygon(&plane, &p, &mut cf, &mut cb, &mut fr, &mut bk);
            node.polygons.extend(cf);
            node.polygons.extend(cb);
            front.extend(fr);
            back.extend(bk);
        }
        if !front.is_empty() {
            let child = node.front.get_or_insert_with(|| Box::new(BspNode::new(Vec::new())));
            stack.push((child, front));
        }
        if !back.is_empty() {
            let child = node.back.get_or_insert_with(|| Box::new(BspNode::new(Vec::new())));
            stack.push((child, back));
        }
    }
}

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

fn pos_key(p: Vec3) -> (i64, i64, i64) {
    let s = 1_000_000.0; // 1e-6 mm
    ((p.x * s).round() as i64, (p.y * s).round() as i64, (p.z * s).round() as i64)
}

fn plane_key(p: &Plane) -> (i64, i64, i64, i64) {
    let s = 1_000_000.0;
    (
        (p.normal.x * s).round() as i64,
        (p.normal.y * s).round() as i64,
        (p.normal.z * s).round() as i64,
        (p.w * s).round() as i64,
    )
}

/// Undo a plane group's internal triangulation (a "fan from centre" cap, or
/// a diagonal-split quad wall) back into a single polygon, by dropping edges
/// shared by two triangles of the group and chaining what's left into one
/// boundary loop. This matters because BSP-CSG clips whole input polygons:
/// if a primitive's own flat face is fed in pre-split by an arbitrary
/// internal diagonal, a neighbouring face clipped at a slightly different
/// point along that same physical edge produces a T-vertex the strict
/// manifold check (and downstream slicers) will flag, even though the
/// surface has no real gap. Returns None (caller falls back to per-triangle
/// polygons) if the group isn't a single simple loop -- e.g. it is itself
/// the result of an earlier boolean op and legitimately has multiple
/// boundary components (a face with a hole in it).
fn try_merge_group(mesh: &Mesh, tri_idxs: &[usize], plane: Plane, tag: u32) -> Option<Polygon> {
    // BTreeMap, not HashMap: `start` below is picked by iteration order, and
    // `HashMap`'s is randomised per instance, which would make boolean output
    // vary between runs of identical input (spec section 5.2 requires
    // deterministic evaluation).
    use std::collections::{BTreeMap, BTreeSet};
    let mut edge_count: BTreeMap<((i64, i64, i64), (i64, i64, i64)), i32> = BTreeMap::new();
    let mut pos_of: BTreeMap<(i64, i64, i64), Vec3> = BTreeMap::new();
    for &ti in tri_idxs {
        let t = mesh.indices[ti];
        let pts = [mesh.positions[t[0] as usize], mesh.positions[t[1] as usize], mesh.positions[t[2] as usize]];
        for k in 0..3 {
            let (a, b) = (pts[k], pts[(k + 1) % 3]);
            let (ka, kb) = (pos_key(a), pos_key(b));
            pos_of.insert(ka, a);
            pos_of.insert(kb, b);
            *edge_count.entry((ka, kb)).or_insert(0) += 1;
        }
    }
    let mut next: BTreeMap<(i64, i64, i64), (i64, i64, i64)> = BTreeMap::new();
    for (&(ka, kb), &c) in edge_count.iter() {
        let rev = edge_count.get(&(kb, ka)).copied().unwrap_or(0);
        if c == 1 && rev == 0 {
            if next.insert(ka, kb).is_some() {
                return None; // non-simple boundary (a vertex used by >1 boundary edge)
            }
        } else if c != rev {
            return None; // inconsistent local triangulation; don't guess
        }
    }
    if next.is_empty() {
        return None;
    }
    let start = *next.keys().next().unwrap();
    let mut verts = Vec::new();
    let mut cur = start;
    let mut visited = BTreeSet::new();
    loop {
        if !visited.insert(cur) {
            return None;
        }
        verts.push(pos_of[&cur]);
        cur = *next.get(&cur)?;
        if cur == start {
            break;
        }
    }
    if verts.len() != next.len() {
        return None; // boundary has more than one loop (e.g. a face with a hole)
    }
    if !is_convex_loop(&verts, &plane) {
        // Both `split_polygon` and `polygons_to_mesh` assume convexity (the
        // latter fan-triangulates from vertex 0). A concave merged face -- an
        // L-shaped face left behind by an earlier boolean, say -- would be
        // silently mis-split and mis-triangulated, so fall back to feeding
        // this group's triangles in individually.
        return None;
    }
    Some(Polygon { vertices: verts, plane, tag })
}

/// True if the loop turns the same way at every vertex when viewed along the
/// plane normal. Exactly-collinear vertices (a fan triangulation's midpoints)
/// are tolerated: they are harmless for both splitting and fan-triangulation.
fn is_convex_loop(verts: &[Vec3], plane: &Plane) -> bool {
    let n = verts.len();
    if n < 3 {
        return false;
    }
    let mut sign = 0.0f64;
    for i in 0..n {
        let a = verts[i];
        let b = verts[(i + 1) % n];
        let c = verts[(i + 2) % n];
        let turn = (b - a).cross(c - b).dot(plane.normal);
        if turn.abs() < 1e-12 {
            continue;
        }
        if sign == 0.0 {
            sign = turn.signum();
        } else if turn.signum() != sign {
            return false;
        }
    }
    true
}

pub fn debug_mesh_to_polygons(mesh: &Mesh) -> Vec<Vec<Vec3>> {
    mesh_to_polygons(mesh).into_iter().map(|p| p.vertices).collect()
}

/// What makes two triangles part of the same face: the plane they lie in and
/// the body they came from.
type FaceGroup = ((i64, i64, i64, i64), u32);

fn mesh_to_polygons(mesh: &Mesh) -> Vec<Polygon> {
    let planes: Vec<Option<Plane>> = mesh
        .indices
        .iter()
        .map(|t| {
            Plane::from_points(
                mesh.positions[t[0] as usize],
                mesh.positions[t[1] as usize],
                mesh.positions[t[2] as usize],
            )
        })
        .collect();
    // Grouped by plane *and* tag: coplanar faces of two different bodies are
    // not one face, and merging them would lose which body each part came from.
    let mut groups: std::collections::BTreeMap<FaceGroup, Vec<usize>> = std::collections::BTreeMap::new();
    for (i, p) in planes.iter().enumerate() {
        if let Some(pl) = p {
            groups.entry((plane_key(pl), mesh.tag(i))).or_default().push(i);
        }
    }
    let mut polygons = Vec::new();
    for (&(_, tag), tri_idxs) in groups.iter() {
        let plane = planes[tri_idxs[0]].unwrap();
        if tri_idxs.len() > 1 {
            if let Some(merged) = try_merge_group(mesh, tri_idxs, plane, tag) {
                polygons.push(merged);
                continue;
            }
        }
        for &i in tri_idxs {
            let t = mesh.indices[i];
            let (a, b, c) =
                (mesh.positions[t[0] as usize], mesh.positions[t[1] as usize], mesh.positions[t[2] as usize]);
            polygons.push(Polygon { vertices: vec![a, b, c], plane, tag });
        }
    }
    polygons
}

pub fn debug_roundtrip(mesh: &Mesh) -> Mesh {
    polygons_to_mesh(&mesh_to_polygons(mesh))
}

/// Whether `build` can chain this mesh's faces instead of classifying every one
/// of them against every plane -- true exactly when no face's plane divides
/// another, which is what a convex solid is. Exposed for the test that holds
/// the shortcut to that meaning.
pub fn debug_splits_nothing(mesh: &Mesh) -> bool {
    non_splitting_order(&mesh_to_polygons(mesh)).is_some()
}

fn polygons_to_mesh(polys: &[Polygon]) -> Mesh {
    let mut mesh = Mesh::new();
    for poly in polys {
        // Fan-triangulate; every polygon here is convex (a plane-clipped convex
        // input stays convex, and `try_merge_group` rejects concave merges), so
        // a fan from vertex 0 is always valid. Start the fan at a vertex that
        // actually turns: a merged face can carry collinear T-junction vertices,
        // and fanning from one of those emits zero-area triangles whose edges
        // then break the manifold check.
        let n = poly.vertices.len();
        if n < 3 {
            continue;
        }
        // A fan from vertex k produces a zero-area triangle whenever k lies on
        // the supporting line of one of the edges the fan spans. For a convex
        // loop that happens exactly when k is inside a collinear run or is
        // adjacent to a vertex that is, so require k and both its neighbours to
        // be genuine corners.
        let turns: Vec<bool> = (0..n)
            .map(|k| {
                let a = poly.vertices[(k + n - 1) % n];
                let b = poly.vertices[k];
                let c = poly.vertices[(k + 1) % n];
                (b - a).cross(c - b).length() > 1e-12
            })
            .collect();
        let apex = (0..n)
            .find(|&k| turns[k] && turns[(k + 1) % n] && turns[(k + n - 1) % n])
            .or_else(|| (0..n).find(|&k| turns[k]))
            .unwrap_or(0);
        for i in 1..n - 1 {
            mesh.push_tagged_triangle(
                poly.vertices[apex],
                poly.vertices[(apex + i) % n],
                poly.vertices[(apex + i + 1) % n],
                poly.tag,
            );
        }
    }
    mesh
}

/// If no plane of `polygons` divides any of them, the planes grouped by the
/// order they should be chained in; `None` if any plane splits something and
/// the general build must do the work.
///
/// This is the convex case, and it is not an exotic one: a sphere, a cylinder,
/// a box, a cap, a prism -- every round primitive the application offers is
/// convex, and a convex body is the worst case for the classic BSP build. No
/// face of a sphere divides the others (they are all behind it), so the tree is
/// a chain, and the general build re-classifies every remaining face at every
/// level to discover that: quadratic in the face count, and the reason a
/// spherical cap at 128 segments spent a second of its second inside
/// `BspNode::build` alone. Proving the same thing through the bounding-volume
/// hierarchy costs one query per distinct plane.
fn non_splitting_order(polygons: &[Polygon]) -> Option<Vec<Vec<usize>>> {
    /// Not worth the hierarchy: the general build is already linear here.
    const MIN: usize = 64;
    if polygons.len() < MIN {
        return None;
    }
    let tree = BoxTree::new(polygons)?;

    // Grouped by plane, in first-appearance order, so the chain takes the
    // planes in the order the general build would have taken them.
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut index: std::collections::BTreeMap<(i64, i64, i64, i64), usize> = std::collections::BTreeMap::new();
    for (i, p) in polygons.iter().enumerate() {
        let slot = *index.entry(unoriented_plane_key(&p.plane)).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[slot].push(i);
    }
    for group in &groups {
        if !tree.all_behind(&polygons[group[0]].plane, polygons) {
            return None;
        }
    }
    Some(groups)
}

fn op(a: &Mesh, b: &Mesh, kind: BoolOp) -> Mesh {
    let mut na = BspNode::new(mesh_to_polygons(a));
    let mut nb = BspNode::new(mesh_to_polygons(b));
    let polys = match kind {
        BoolOp::Union => {
            na.clip_to(&nb);
            nb.clip_to(&na);
            nb.invert();
            nb.clip_to(&na);
            nb.invert();
            na.build(nb.all_polygons());
            na.all_polygons()
        }
        BoolOp::Subtract => {
            na.invert();
            na.clip_to(&nb);
            nb.clip_to(&na);
            nb.invert();
            nb.clip_to(&na);
            nb.invert();
            na.build(nb.all_polygons());
            na.invert();
            na.all_polygons()
        }
        BoolOp::Intersect => {
            na.invert();
            nb.clip_to(&na);
            nb.invert();
            na.clip_to(&nb);
            nb.clip_to(&na);
            na.build(nb.all_polygons());
            na.invert();
            na.all_polygons()
        }
    };
    // The BSP clips whole polygons, which leaves T-junctions wherever two
    // polygons sharing an edge were split at different points along it; heal
    // them here so every boolean result -- including one feeding the next
    // boolean in a chain -- is edge-manifold. See `repair`.
    crate::repair::heal(&polygons_to_mesh(&polys))
}

enum BoolOp {
    Union,
    Subtract,
    Intersect,
}

pub fn union(a: &Mesh, b: &Mesh) -> Mesh {
    op(a, b, BoolOp::Union)
}

pub fn subtract(a: &Mesh, b: &Mesh) -> Mesh {
    op(a, b, BoolOp::Subtract)
}

pub fn intersect(a: &Mesh, b: &Mesh) -> Mesh {
    op(a, b, BoolOp::Intersect)
}
