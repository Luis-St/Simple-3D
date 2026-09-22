//! A bounding volume hierarchy over a mesh's triangles, so a ray asks a few
//! dozen triangles where it used to ask every one.

use super::ray::ray_triangle;
use crate::render::map_in_order;
use simple3d_geom::{Mesh, Vec3};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

/// Below this a mesh is simply walked: building the tree costs more than the
/// handful of casts a small body ever gets.
pub(super) const MIN_TRIANGLES: usize = 2_048;

/// How many triangles a leaf holds.
const LEAF: usize = 4;

pub(crate) struct Bvh {
    nodes: Vec<Node>,
    /// Triangle indices, in the order the leaves refer to them.
    order: Vec<u32>,
}

#[derive(Clone, Copy)]
struct Node {
    lo: Vec3,
    hi: Vec3,
    /// A leaf's first entry in `order`, or an inner node's first child -- its
    /// second child is the node after that.
    first: u32,
    /// How many triangles a leaf holds; 0 for an inner node.
    count: u32,
}

impl Bvh {
    pub(crate) fn build(mesh: &Mesh) -> Bvh {
        // Each triangle's box and centre, worked out once: the build asks for
        // them at every level of the tree, and looking the corners up again
        // each time made building the tree for a large import take seconds.
        let corners = |index: usize| mesh.indices[index].map(|corner| mesh.positions[corner as usize]);
        let boxes: Vec<(Vec3, Vec3)> = map_in_order(mesh.indices.len(), |index| {
            let [a, b, c] = corners(index);
            (a.min(b).min(c), a.max(b).max(c))
        });
        let centres: Vec<[f64; 3]> = map_in_order(mesh.indices.len(), |index| {
            let [a, b, c] = corners(index);
            let centre = (a + b + c) * (1.0 / 3.0);
            [centre.x, centre.y, centre.z]
        });
        let mut order: Vec<u32> = (0..mesh.indices.len() as u32).collect();
        let (boxes, centres) = (&boxes[..], &centres[..]);

        // The top of the tree on this thread, down to a few stretches of
        // triangles per core; each of those is then a subtree of its own,
        // built side by side and joined on afterwards. The first cast at a
        // large import waits for the whole build, so this is time the user
        // sees.
        let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
        let chunk = (order.len() / (cores * 4)).max(8_192);
        let mut nodes = vec![Node::blank()];
        let mut deferred = Vec::new();
        grow(&mut order, boxes, centres, &mut nodes, chunk, &mut deferred);

        deferred.sort_unstable_by_key(|&(_, from, _)| from);
        let mut slices = Vec::with_capacity(deferred.len());
        let (mut rest, mut consumed) = (&mut order[..], 0);
        for &(at, from, to) in &deferred {
            let (_, tail) = rest.split_at_mut(from - consumed);
            let (mine, tail) = tail.split_at_mut(to - from);
            slices.push((at, from, mine));
            (rest, consumed) = (tail, to);
        }
        let built: Vec<(usize, usize, Vec<Node>)> = std::thread::scope(|scope| {
            let handles: Vec<_> = slices
                .into_iter()
                .map(|(at, from, slice)| {
                    scope.spawn(move || {
                        let mut local = vec![Node::blank()];
                        grow(slice, boxes, centres, &mut local, 0, &mut Vec::new());
                        (at, from, local)
                    })
                })
                .collect();
            handles.into_iter().map(|handle| handle.join().expect("a tree-building thread panicked")).collect()
        });
        // A subtree's root takes the place left for it; the rest go on the end.
        // Its children were numbered from its own root and its leaves from the
        // start of its own stretch, so both are moved to where they now are.
        for (at, from, local) in built {
            let base = nodes.len();
            nodes[at] = local[0].moved(base, from);
            nodes.extend(local[1..].iter().map(|node| node.moved(base, from)));
        }
        Bvh { nodes, order }
    }

    /// The nearest hit along the ray, exactly as walking every triangle with
    /// `ray_triangle` finds it: the same test on the same triangles, only
    /// skipping the ones in boxes the ray cannot reach before the best hit so
    /// far.
    pub(crate) fn nearest(&self, mesh: &Mesh, origin: Vec3, dir: Vec3) -> Option<f64> {
        let mut nearest: Option<f64> = None;
        let mut stack = vec![0u32];
        while let Some(at) = stack.pop() {
            let node = &self.nodes[at as usize];
            let limit = nearest.unwrap_or(f64::INFINITY);
            if enters(origin, dir, node.lo, node.hi, limit).is_none() {
                continue;
            }
            if node.count > 0 {
                let first = node.first as usize;
                for &index in &self.order[first..first + node.count as usize] {
                    let [a, b, c] = mesh.indices[index as usize].map(|corner| mesh.positions[corner as usize]);
                    if let Some(t) = ray_triangle(origin, dir, a, b, c) {
                        if nearest.is_none_or(|best| t < best) {
                            nearest = Some(t);
                        }
                    }
                }
                continue;
            }
            // The nearer child last, so it is looked at first and the best hit
            // shrinks before the farther one is asked.
            let (left, right) = (node.first, node.first + 1);
            let near_left = enters(origin, dir, self.nodes[left as usize].lo, self.nodes[left as usize].hi, limit);
            let near_right = enters(origin, dir, self.nodes[right as usize].lo, self.nodes[right as usize].hi, limit);
            match (near_left, near_right) {
                (Some(l), Some(r)) if l <= r => stack.extend([right, left]),
                (Some(_), Some(_)) => stack.extend([left, right]),
                (Some(_), None) => stack.push(left),
                (None, Some(_)) => stack.push(right),
                (None, None) => {}
            }
        }
        nearest
    }
}

impl Node {
    fn blank() -> Node {
        Node { lo: Vec3::ZERO, hi: Vec3::ZERO, first: 0, count: 0 }
    }

    /// A node of a subtree built on its own, as it reads once the subtree's
    /// nodes after its root are appended at `base` and its leaves cover the
    /// stretch of the whole order starting at `from`.
    fn moved(&self, base: usize, from: usize) -> Node {
        match self.count {
            0 => Node { first: (base + self.first as usize - 1) as u32, ..*self },
            _ => Node { first: self.first + from as u32, ..*self },
        }
    }
}

/// Grow the tree for `order` into `nodes`, whose first entry is its root:
/// split at the median centre along the longest side of the centres' own box,
/// down to leaves of a few triangles.
///
/// A stretch of no more than `defer` triangles is not built but listed in
/// `deferred`, as the node it belongs at and the stretch of `order` it covers,
/// for the caller to build separately.
fn grow(
    order: &mut [u32],
    boxes: &[(Vec3, Vec3)],
    centres: &[[f64; 3]],
    nodes: &mut Vec<Node>,
    defer: usize,
    deferred: &mut Vec<(usize, usize, usize)>,
) {
    // Each entry is a node still to be filled in, and the stretch of `order`
    // it covers.
    let mut pending = vec![(0usize, 0usize, order.len())];
    while let Some((at, from, to)) = pending.pop() {
        if to - from > LEAF && to - from <= defer {
            deferred.push((at, from, to));
            continue;
        }
        let (lo, hi) = order[from..to].iter().fold(empty(), |(lo, hi), &index| {
            let (a, b) = boxes[index as usize];
            (lo.min(a), hi.max(b))
        });
        // A little room round every box: a ray that hits a triangle on its very
        // edge -- where `ray_triangle`'s own tolerance lets it -- must not be
        // turned away by the box the triangle is in.
        let pad = (hi - lo).length() * 1e-9 + 1e-9;
        let pad = Vec3::new(pad, pad, pad);
        let (lo, hi) = (lo - pad, hi + pad);
        if to - from <= LEAF {
            nodes[at] = Node { lo, hi, first: from as u32, count: (to - from) as u32 };
            continue;
        }
        let (clo, chi) = order[from..to].iter().fold(empty(), |(lo, hi), &index| {
            let [x, y, z] = centres[index as usize];
            let p = Vec3::new(x, y, z);
            (lo.min(p), hi.max(p))
        });
        let span = chi - clo;
        let axis = if span.x >= span.y && span.x >= span.z {
            0
        } else if span.y >= span.z {
            1
        } else {
            2
        };
        let middle = (to - from) / 2;
        order[from..to]
            .select_nth_unstable_by(middle, |&a, &b| centres[a as usize][axis].total_cmp(&centres[b as usize][axis]));
        let first = nodes.len();
        nodes.push(Node::blank());
        nodes.push(Node::blank());
        nodes[at] = Node { lo, hi, first: first as u32, count: 0 };
        pending.push((first, from, from + middle));
        pending.push((first + 1, from + middle, to));
    }
}

/// Where the ray enters the box, if it does before `limit`. The slab test of
/// `ray_box`, with the far end cut short at the best hit so far.
fn enters(origin: Vec3, dir: Vec3, lo: Vec3, hi: Vec3, limit: f64) -> Option<f64> {
    let (mut near, mut far) = (0.0_f64, limit);
    for axis in 0..3 {
        let (o, d, l, h) = (component(origin, axis), component(dir, axis), component(lo, axis), component(hi, axis));
        if d.abs() < 1e-12 {
            if o < l || o > h {
                return None;
            }
            continue;
        }
        let (mut a, mut b) = ((l - o) / d, (h - o) / d);
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        near = near.max(a);
        far = far.min(b);
        if near > far {
            return None;
        }
    }
    Some(near)
}

/// A box that holds nothing, for a fold to grow from.
fn empty() -> (Vec3, Vec3) {
    let (inf, neg) = (f64::INFINITY, f64::NEG_INFINITY);
    (Vec3::new(inf, inf, inf), Vec3::new(neg, neg, neg))
}

fn component(v: Vec3, axis: usize) -> f64 {
    match axis {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

/// The tree for a mesh, built the first time a ray is cast at it and kept for
/// as long as the mesh lives.
///
/// Keyed by the mesh's address, and holding a `Weak` to it: a `Weak` keeps the
/// allocation from being freed, so no other mesh can be given that address
/// while the entry remembers it, and a mesh that has been dropped is seen to be
/// gone -- and its tree let go of -- the next time a ray is cast at anything.
pub(crate) fn tree_for(mesh: &Arc<Mesh>) -> Arc<Bvh> {
    type Trees = HashMap<usize, (Weak<Mesh>, Arc<Bvh>)>;
    static TREES: Mutex<Option<Trees>> = Mutex::new(None);
    let key = Arc::as_ptr(mesh) as usize;
    {
        let mut trees = TREES.lock().expect("the tree cache lock");
        let trees = trees.get_or_insert_with(HashMap::new);
        trees.retain(|_, (weak, _)| weak.strong_count() > 0);
        if let Some((_, tree)) = trees.get(&key) {
            return tree.clone();
        }
    }
    // Built outside the lock: a large mesh takes a moment, and nothing else
    // needs to wait for it.
    let tree = Arc::new(Bvh::build(mesh));
    let mut trees = TREES.lock().expect("the tree cache lock");
    trees.get_or_insert_with(HashMap::new).insert(key, (Arc::downgrade(mesh), tree.clone()));
    tree
}
