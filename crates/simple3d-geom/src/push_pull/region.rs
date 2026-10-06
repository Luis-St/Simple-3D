//! The flat region of a mesh around one triangle: the face a push/pull starts from.

use super::*;
use crate::mesh::{FastMap, Mesh};
use crate::vec3::Vec3;

/// Neighbours meeting a region at less than this many degrees make it part of a curved surface: a
/// facet of a cylinder's side rather than a face of its own.
pub const CURVED_DEG: f64 = 20.0;

/// A flat, edge-connected region of one source's faces in a mesh, with its boundary.
#[derive(Clone, Debug)]
pub struct FlatFace {
    /// The region's triangles, by index into the mesh.
    pub triangles: Vec<usize>,
    /// Outward, as the triangles are wound.
    pub normal: Vec3,
    /// The middle of the region's box, on its plane.
    pub centre: Vec3,
    /// The boundary loops, wound like the triangles: the outer one counter-clockwise about `normal`.
    pub loops: Vec<Vec<Vec3>>,
    /// Whether the region is a facet of a curved surface (see [`CURVED_DEG`]).
    pub curved: bool,
}

type Key = (i64, i64, i64);

fn key(p: Vec3) -> Key {
    let s = 1e5;
    ((p.x * s).round() as i64, (p.y * s).round() as i64, (p.z * s).round() as i64)
}

/// The flat region `triangle` belongs to. `None` for a degenerate triangle, or a region whose boundary
/// cannot be followed (it touches itself at a corner).
pub fn flat_face(mesh: &Mesh, triangle: usize) -> Option<FlatFace> {
    let tri = *mesh.indices.get(triangle)?;
    let normal = mesh.triangle_normal(tri);
    if !normal.length().is_finite() || normal.length() < 0.5 {
        return None;
    }
    let anchor = mesh.positions[tri[0] as usize];
    let keys: Vec<Key> = mesh.positions.iter().map(|&p| key(p)).collect();
    let tri_keys = |t: usize| mesh.indices[t].map(|v| keys[v as usize]);
    // Every undirected edge and the triangles using it.
    let mut edges: FastMap<(Key, Key), Vec<usize>> = FastMap::default();
    for t in 0..mesh.indices.len() {
        let k = tri_keys(t);
        for e in 0..3 {
            let (a, b) = (k[e], k[(e + 1) % 3]);
            edges.entry(if a < b { (a, b) } else { (b, a) }).or_default().push(t);
        }
    }
    // One object's face only: a flush face of another object is a face of its own (issue 73), even
    // where the union joined the two.
    let source = mesh.source(triangle);
    let coplanar = |t: usize| {
        let n = mesh.triangle_normal(mesh.indices[t]);
        let corners = mesh.corners(mesh.indices[t]);
        n.dot(normal) > 1.0 - 1e-6 && corners.iter().all(|&p| (p - anchor).dot(normal).abs() < 1e-4)
    };
    let joins = |t: usize| mesh.source(t) == source && coplanar(t);

    let mut inside = vec![false; mesh.indices.len()];
    let mut stack = vec![triangle];
    inside[triangle] = true;
    let mut triangles = Vec::new();
    while let Some(t) = stack.pop() {
        triangles.push(t);
        let k = tri_keys(t);
        for e in 0..3 {
            let (a, b) = (k[e], k[(e + 1) % 3]);
            for &other in &edges[&if a < b { (a, b) } else { (b, a) }] {
                if !inside[other] && joins(other) {
                    inside[other] = true;
                    stack.push(other);
                }
            }
        }
    }

    // The boundary: directed edges whose reverse is not in the region.
    let mut directed: FastMap<(Key, Key), u32> = FastMap::default();
    let mut at: FastMap<Key, Vec3> = FastMap::default();
    for &t in &triangles {
        let k = tri_keys(t);
        for e in 0..3 {
            *directed.entry((k[e], k[(e + 1) % 3])).or_insert(0) += 1;
            at.insert(k[e], mesh.positions[mesh.indices[t][e] as usize]);
        }
    }
    let mut next: std::collections::BTreeMap<Key, Key> = std::collections::BTreeMap::new();
    let mut curved = false;
    let cos_curved = CURVED_DEG.to_radians().cos();
    for (&(a, b), _) in directed.iter().filter(|(&(a, b), _)| !directed.contains_key(&(b, a))) {
        if next.insert(a, b).is_some() {
            return None;
        }
        let across = &edges[&if a < b { (a, b) } else { (b, a) }];
        curved |= across
            .iter()
            .filter(|&&t| !inside[t] && !coplanar(t))
            .any(|&t| mesh.triangle_normal(mesh.indices[t]).dot(normal) > cos_curved);
    }
    let mut loops = Vec::new();
    while let Some((&start, _)) = next.iter().next() {
        let mut points = Vec::new();
        let mut current = start;
        loop {
            points.push(at[&current]);
            current = next.remove(&current)?;
            if current == start {
                break;
            }
            if points.len() > directed.len() {
                return None;
            }
        }
        loops.push(points);
    }
    if loops.is_empty() {
        return None;
    }
    let all = loops.iter().flatten().copied();
    let (lo, hi) = crate::aabb::bounds_of(all)?;
    let middle = (lo + hi) * 0.5;
    let centre = middle - normal * (middle - anchor).dot(normal);
    Some(FlatFace { triangles, normal, centre, loops, curved })
}

impl FlatFace {
    /// The boundary flattened onto axes `u`, `v` about `origin`, which should lie on the face's plane
    /// with `u x v` along its normal. `None` when no loop encloses the others.
    pub fn outline(&self, origin: Vec3, u: Vec3, v: Vec3) -> Option<Outline> {
        let flat: Vec<Vec<[f64; 2]>> = self
            .loops
            .iter()
            .map(|points| points.iter().map(|&p| [(p - origin).dot(u), (p - origin).dot(v)]).collect())
            .collect();
        // The outer loop is the one enclosing the most area; the rest are holes in it.
        let outer = (0..flat.len()).max_by(|&a, &b| area(&flat[a]).abs().total_cmp(&area(&flat[b]).abs()))?;
        if area(&flat[outer]) <= 0.0 {
            return None;
        }
        let holes = flat.iter().enumerate().filter(|(i, _)| *i != outer).map(|(_, l)| l.clone()).collect();
        Outline::new(flat[outer].clone(), holes)
    }
}
