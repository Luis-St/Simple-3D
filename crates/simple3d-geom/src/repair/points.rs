//! A bounding-box hierarchy over the vertices of a mesh, for finding the ones
//! that lie along an edge.

use crate::vec3::Vec3;

/// Below this many vertices a node stops dividing.
const LEAF: usize = 8;

/// Vertices sorted into boxes that halve at the median of their widest axis,
/// so a box holds the same number of vertices wherever the mesh is dense.
///
/// This replaces a uniform grid, and the density is why. A boolean between two
/// bodies that share a face leaves its vertices crowded along the seam, ten
/// thousand of them to a half-millimetre cell, while the rest of the surface
/// has a handful to a cell; every edge through the seam was tested against the
/// whole crowd, and one T-junction pass over the union of an imported model's
/// coloured parts made some ten billion such tests. The hierarchy tests an edge
/// against the few boxes it actually passes through, however small those have
/// to be.
pub(crate) struct PointTree {
    nodes: Vec<Node>,
    /// Vertex indices, permuted so that each leaf owns a contiguous run.
    order: Vec<u32>,
}

struct Node {
    lo: Vec3,
    hi: Vec3,
    /// Two children for a split, one past the other; a run of `order` for a
    /// leaf.
    split: bool,
    a: u32,
    b: u32,
}

impl PointTree {
    pub(crate) fn new(positions: &[Vec3]) -> PointTree {
        let mut tree = PointTree { nodes: Vec::new(), order: (0..positions.len() as u32).collect() };
        if positions.is_empty() {
            return tree;
        }
        tree.nodes.push(Node { lo: Vec3::ZERO, hi: Vec3::ZERO, split: false, a: 0, b: 0 });
        // Iterative: a mesh can have a million vertices.
        let mut stack: Vec<(usize, usize, usize)> = vec![(0, positions.len(), 0)];
        while let Some((from, to, slot)) = stack.pop() {
            let mut lo = positions[tree.order[from] as usize];
            let mut hi = lo;
            for &i in &tree.order[from + 1..to] {
                lo = lo.min(positions[i as usize]);
                hi = hi.max(positions[i as usize]);
            }
            if to - from <= LEAF {
                tree.nodes[slot] = Node { lo, hi, split: false, a: from as u32, b: to as u32 };
                continue;
            }
            let size = hi - lo;
            let axis = |p: Vec3| {
                if size.x >= size.y && size.x >= size.z {
                    p.x
                } else if size.y >= size.z {
                    p.y
                } else {
                    p.z
                }
            };
            let mid = from + (to - from) / 2;
            tree.order[from..to].select_nth_unstable_by(mid - from, |&a, &b| {
                axis(positions[a as usize]).total_cmp(&axis(positions[b as usize]))
            });
            let left = tree.nodes.len();
            tree.nodes.push(Node { lo: Vec3::ZERO, hi: Vec3::ZERO, split: false, a: 0, b: 0 });
            tree.nodes.push(Node { lo: Vec3::ZERO, hi: Vec3::ZERO, split: false, a: 0, b: 0 });
            tree.nodes[slot] = Node { lo, hi, split: true, a: left as u32, b: 0 };
            stack.push((from, mid, left));
            stack.push((mid, to, left + 1));
        }
        tree
    }

    /// Every vertex in a box that the segment from `a` to `b`, thickened by
    /// `tol`, passes through: a superset of the vertices within `tol` of it,
    /// which the caller measures exactly. `stack` is scratch space, kept by the
    /// caller so a query per edge allocates nothing.
    pub(crate) fn near_segment(&self, a: Vec3, b: Vec3, tol: f64, stack: &mut Vec<u32>, mut visit: impl FnMut(u32)) {
        if self.nodes.is_empty() {
            return;
        }
        stack.clear();
        stack.push(0);
        while let Some(at) = stack.pop() {
            let node = &self.nodes[at as usize];
            if !crosses(a, b, node.lo - Vec3::splat(tol), node.hi + Vec3::splat(tol)) {
                continue;
            }
            if node.split {
                stack.push(node.a);
                stack.push(node.a + 1);
            } else {
                self.order[node.a as usize..node.b as usize].iter().for_each(|&v| visit(v));
            }
        }
    }
}

/// Whether the segment from `a` to `b` meets the box: the slab test, clipping
/// the segment's parameter range to each axis in turn.
fn crosses(a: Vec3, b: Vec3, lo: Vec3, hi: Vec3) -> bool {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (start, step, lo, hi) in [(a.x, d.x, lo.x, hi.x), (a.y, d.y, lo.y, hi.y), (a.z, d.z, lo.z, hi.z)] {
        if step == 0.0 {
            if start < lo || start > hi {
                return false;
            }
            continue;
        }
        let (mut near, mut far) = ((lo - start) / step, (hi - start) / step);
        if near > far {
            std::mem::swap(&mut near, &mut far);
        }
        t0 = t0.max(near);
        t1 = t1.min(far);
        if t0 > t1 {
            return false;
        }
    }
    true
}
