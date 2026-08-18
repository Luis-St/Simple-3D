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
}

impl Polygon {
    fn flip(&self) -> Polygon {
        let mut v = self.vertices.clone();
        v.reverse();
        Polygon { vertices: v, plane: self.plane.flip() }
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
                front.push(Polygon { vertices: f, plane: poly.plane });
            }
            if b.len() >= 3 {
                back.push(Polygon { vertices: b, plane: poly.plane });
            }
        }
    }
}

struct BspNode {
    plane: Option<Plane>,
    front: Option<Box<BspNode>>,
    back: Option<Box<BspNode>>,
    polygons: Vec<Polygon>,
}

impl BspNode {
    fn new(polygons: Vec<Polygon>) -> BspNode {
        let mut node = BspNode { plane: None, front: None, back: None, polygons: Vec::new() };
        if !polygons.is_empty() {
            node.build(polygons);
        }
        node
    }

    fn invert(&mut self) {
        for p in self.polygons.iter_mut() {
            *p = p.flip();
        }
        if let Some(plane) = &mut self.plane {
            *plane = plane.flip();
        }
        if let Some(f) = &mut self.front {
            f.invert();
        }
        if let Some(b) = &mut self.back {
            b.invert();
        }
        std::mem::swap(&mut self.front, &mut self.back);
    }

    fn clip_polygons(&self, polygons: &[Polygon]) -> Vec<Polygon> {
        let Some(plane) = self.plane else {
            return polygons.to_vec();
        };
        let mut cf = Vec::new();
        let mut cb = Vec::new();
        let mut front = Vec::new();
        let mut back = Vec::new();
        for p in polygons {
            split_polygon(&plane, p, &mut cf, &mut cb, &mut front, &mut back);
        }
        front.extend(cf);
        back.extend(cb);
        let mut front = if let Some(f) = &self.front { f.clip_polygons(&front) } else { front };
        let back = if let Some(b) = &self.back { b.clip_polygons(&back) } else { Vec::new() };
        front.extend(back);
        front
    }

    fn clip_to(&mut self, other: &BspNode) {
        self.polygons = other.clip_polygons(&self.polygons);
        if let Some(f) = &mut self.front {
            f.clip_to(other);
        }
        if let Some(b) = &mut self.back {
            b.clip_to(other);
        }
    }

    fn all_polygons(&self) -> Vec<Polygon> {
        let mut result = self.polygons.clone();
        if let Some(f) = &self.front {
            result.extend(f.all_polygons());
        }
        if let Some(b) = &self.back {
            result.extend(b.all_polygons());
        }
        result
    }

    fn build(&mut self, polygons: Vec<Polygon>) {
        if polygons.is_empty() {
            return;
        }
        if self.plane.is_none() {
            self.plane = Some(polygons[0].plane);
        }
        let plane = self.plane.unwrap();
        let mut front = Vec::new();
        let mut back = Vec::new();
        for p in polygons {
            let mut cf = Vec::new();
            let mut cb = Vec::new();
            let mut fr = Vec::new();
            let mut bk = Vec::new();
            split_polygon(&plane, &p, &mut cf, &mut cb, &mut fr, &mut bk);
            self.polygons.extend(cf);
            self.polygons.extend(cb);
            front.extend(fr);
            back.extend(bk);
        }
        if !front.is_empty() {
            self.front.get_or_insert_with(|| Box::new(BspNode::new(Vec::new()))).build(front);
        }
        if !back.is_empty() {
            self.back.get_or_insert_with(|| Box::new(BspNode::new(Vec::new()))).build(back);
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
fn try_merge_group(mesh: &Mesh, tri_idxs: &[usize], plane: Plane) -> Option<Polygon> {
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
    Some(Polygon { vertices: verts, plane })
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
    let mut groups: std::collections::BTreeMap<(i64, i64, i64, i64), Vec<usize>> = std::collections::BTreeMap::new();
    for (i, p) in planes.iter().enumerate() {
        if let Some(pl) = p {
            groups.entry(plane_key(pl)).or_default().push(i);
        }
    }
    let mut polygons = Vec::new();
    for tri_idxs in groups.values() {
        let plane = planes[tri_idxs[0]].unwrap();
        if tri_idxs.len() > 1 {
            if let Some(merged) = try_merge_group(mesh, tri_idxs, plane) {
                polygons.push(merged);
                continue;
            }
        }
        for &i in tri_idxs {
            let t = mesh.indices[i];
            let (a, b, c) =
                (mesh.positions[t[0] as usize], mesh.positions[t[1] as usize], mesh.positions[t[2] as usize]);
            polygons.push(Polygon { vertices: vec![a, b, c], plane });
        }
    }
    polygons
}

pub fn debug_roundtrip(mesh: &Mesh) -> Mesh {
    polygons_to_mesh(&mesh_to_polygons(mesh))
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
            mesh.push_triangle(poly.vertices[apex], poly.vertices[(apex + i) % n], poly.vertices[(apex + i + 1) % n]);
        }
    }
    mesh
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
