//! A bounding-box hierarchy over a mesh's vertices, for finding those along an edge.

use crate::vec3::Vec3;

/// Below this many vertices a node stops dividing.
const LEAF: usize = 8;

/// Vertices in boxes halved at the median of their widest axis, so boxes hold equal counts however
/// dense the mesh. Replaced a uniform grid, which tested seam edges against crowds of thousands of
/// vertices (billions of tests on an imported model).
pub(crate) struct PointTree {
    nodes: Vec<Node>,
    /// Vertex indices, permuted so each leaf owns a contiguous run.
    order: Vec<u32>,
}

struct Node {
    lo: Vec3,
    hi: Vec3,
    /// Two consecutive children when split; a run of `order` for a leaf.
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

    /// Every vertex in boxes the segment `a`-`b`, thickened by `tol`, passes through: a superset the
    /// caller measures exactly. `stack` is reused so queries do not allocate.
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

/// Whether segment `a`-`b` meets the box: the slab test.
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
