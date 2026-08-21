//! Post-boolean mesh repair.
//!
//! A BSP boolean clips whole polygons against the *other* solid's tree, so two
//! polygons that share a physical edge are not guaranteed to be split at the
//! same points along it: a plane of B may cut A's top face while leaving A's
//! side face (entirely on one side of that plane) untouched. The shared edge
//! then has three vertices on one side and two on the other -- a T-junction.
//! The surface has no gap, but the mesh is not edge-manifold, and slicers
//! reject it. This is inherent to the algorithm, not a transcription bug, and
//! it is why `subtract`/`intersect` used to fail the manifold tests.
//!
//! `heal` fixes it after the fact, which is both simpler and more robust than
//! trying to make the BSP produce matched splits: weld coincident vertices with
//! a real tolerance, then give every triangle the vertices that lie on its own
//! edges, in one pass over the mesh the weld produced. One pass, not a loop
//! until nothing is left to split -- see `split_t_junctions` for what the loop
//! did to a finely tessellated boolean.

use crate::mesh::Mesh;
use crate::vec3::Vec3;
use std::collections::HashMap;

/// Positions closer than this are the same point. Boolean intersection points
/// are computed from `f64` plane arithmetic, so two evaluations of the same
/// physical point agree to ~1e-12mm; 1e-6mm is far below any dimension a user
/// can enter and far above that noise.
pub const WELD_TOL: f64 = 1e-6;

type Cell = (i64, i64, i64);

fn cell_of(p: Vec3, size: f64) -> Cell {
    ((p.x / size).floor() as i64, (p.y / size).floor() as i64, (p.z / size).floor() as i64)
}

/// Merge vertices within `tol` of each other. Unlike `Mesh::weld`, which
/// buckets by rounding and so can miss a pair that straddles a bucket
/// boundary, this checks the 27 neighbouring cells and compares actual
/// distances, which is what makes the manifold check trustworthy.
pub fn weld_tolerant(mesh: &Mesh, tol: f64) -> Mesh {
    let size = tol * 2.0;
    let mut buckets: HashMap<Cell, Vec<u32>> = HashMap::new();
    let mut positions: Vec<Vec3> = Vec::with_capacity(mesh.positions.len());
    let mut remap = vec![0u32; mesh.positions.len()];

    for (i, &p) in mesh.positions.iter().enumerate() {
        let (cx, cy, cz) = cell_of(p, size);
        let mut found = None;
        'search: for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(list) = buckets.get(&(cx + dx, cy + dy, cz + dz)) {
                        for &v in list {
                            if (positions[v as usize] - p).length() <= tol {
                                found = Some(v);
                                break 'search;
                            }
                        }
                    }
                }
            }
        }
        let id = match found {
            Some(v) => v,
            None => {
                positions.push(p);
                let v = (positions.len() - 1) as u32;
                buckets.entry((cx, cy, cz)).or_default().push(v);
                v
            }
        };
        remap[i] = id;
    }

    let mut indices = Vec::with_capacity(mesh.indices.len());
    let mut tags = Vec::with_capacity(mesh.indices.len());
    for (i, t) in mesh.indices.iter().enumerate() {
        let t = [remap[t[0] as usize], remap[t[1] as usize], remap[t[2] as usize]];
        if t[0] != t[1] && t[1] != t[2] && t[0] != t[2] {
            indices.push(t);
            tags.push(mesh.tag(i));
        }
    }
    Mesh { positions, indices, tags }
}

/// Drop triangles thinner than `tol`, measured as the smallest distance from a
/// vertex to the opposite edge. Welding has already merged everything closer
/// together than `tol`, so such a triangle is geometrically indistinguishable
/// from a line segment: it contributes no surface, but its three edges are
/// counted by the manifold check and by slicers. They arise wherever a face
/// carrying collinear T-junction vertices gets fan-triangulated.
fn drop_slivers(mesh: Mesh, tol: f64) -> Mesh {
    let pos = &mesh.positions;
    let mut indices: Vec<[u32; 3]> = Vec::with_capacity(mesh.indices.len());
    let mut tags: Vec<u32> = Vec::with_capacity(mesh.indices.len());
    for (i, t) in mesh.indices.iter().enumerate() {
        let (a, b, c) = (pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]);
        let twice_area = (b - a).cross(c - a).length();
        let longest = (b - a).length().max((c - b).length()).max((a - c).length());
        if longest > tol && twice_area / longest > tol {
            indices.push(*t);
            tags.push(mesh.tag(i));
        }
    }
    Mesh { positions: mesh.positions, indices, tags }
}

/// Drop triangle pairs that describe the same three vertices with opposite
/// winding. They are two coincident, oppositely-facing surface patches that
/// enclose no volume, which a boolean can legitimately produce when an
/// operand's face lies exactly on the result's boundary; leaving them in makes
/// every one of their edges used twice in the same direction.
fn cancel_opposite_faces(mesh: Mesh) -> Mesh {
    let key = |t: &[u32; 3]| {
        let mut k = *t;
        k.sort_unstable();
        k
    };
    let mut by_key: HashMap<[u32; 3], Vec<usize>> = HashMap::new();
    for (i, t) in mesh.indices.iter().enumerate() {
        by_key.entry(key(t)).or_default().push(i);
    }
    let mut dead = vec![false; mesh.indices.len()];
    for group in by_key.values() {
        if group.len() < 2 {
            continue;
        }
        // Winding sign relative to the first triangle of the group.
        let mut pos: Vec<usize> = Vec::new();
        let mut neg: Vec<usize> = Vec::new();
        let reference = mesh.indices[group[0]];
        for &i in group {
            if same_winding(&reference, &mesh.indices[i]) {
                pos.push(i);
            } else {
                neg.push(i);
            }
        }
        for _ in 0..pos.len().min(neg.len()) {
            dead[pos.pop().unwrap()] = true;
            dead[neg.pop().unwrap()] = true;
        }
    }
    let (indices, tags) =
        mesh.indices.iter().enumerate().filter(|(i, _)| !dead[*i]).map(|(i, t)| (*t, mesh.tag(i))).unzip();
    Mesh { positions: mesh.positions, indices, tags }
}

fn same_winding(a: &[u32; 3], b: &[u32; 3]) -> bool {
    for r in 0..3 {
        if b[0] == a[r] && b[1] == a[(r + 1) % 3] && b[2] == a[(r + 2) % 3] {
            return true;
        }
    }
    false
}

/// Split every triangle edge that has another vertex of the mesh lying on its
/// interior, so each undirected edge ends up shared by exactly two triangles.
///
/// Every triangle is dealt with **once**, from the vertices the mesh had when
/// the pass started. The obvious implementation instead splits a triangle in
/// two and pushes both halves back into the queue, and that is what shipped
/// first; it is unbounded. A split introduces an *internal* edge from the split
/// point to the opposite corner, that new edge is examined in its turn, and any
/// vertex within a micron of it -- a vertex that was never on the surface's
/// boundary and needs no split at all -- sets off another. On a spherical cap
/// unioned with a plate the cascade turned 50,854 triangles into 2,679,216 and
/// ran out of its own budget, leaving the mesh non-manifold: a boolean at 176
/// segments produced a quarter of a gigabyte of garbage where 25,000 triangles
/// describe the solid.
///
/// Collecting each edge's on-edge vertices up front and triangulating the
/// resulting polygon in one go cannot cascade: the internal edges it creates
/// are never looked at. They need not be. A T-junction is a vertex on the
/// *boundary* between two faces, and the boundary is exactly what the up-front
/// collection sees.
fn split_t_junctions(mesh: Mesh, tol: f64) -> Mesh {
    if mesh.indices.is_empty() {
        return mesh;
    }
    let (lo, hi) = mesh.bounds().unwrap();
    let extent = (hi - lo).x.max((hi - lo).y).max((hi - lo).z);
    let size = (extent / 48.0).max(tol * 16.0);

    let mut grid: HashMap<Cell, Vec<u32>> = HashMap::new();
    for (i, &p) in mesh.positions.iter().enumerate() {
        grid.entry(cell_of(p, size)).or_default().push(i as u32);
    }

    let mut positions = mesh.positions.clone();
    let pos = &mesh.positions;
    let mut indices: Vec<[u32; 3]> = Vec::with_capacity(mesh.indices.len());
    let mut tags: Vec<u32> = Vec::with_capacity(mesh.indices.len());
    // The triangle's boundary with the on-edge vertices spliced into it, reused
    // across triangles rather than reallocated for each.
    let mut loop_: Vec<u32> = Vec::new();
    let mut on_edge: Vec<(f64, u32)> = Vec::new();

    for (i, tri) in mesh.indices.iter().enumerate() {
        let tag = mesh.tag(i);
        loop_.clear();
        for e in 0..3 {
            loop_.push(tri[e]);
            on_edge_vertices(pos, &grid, size, tol, tri, e, &mut on_edge);
            loop_.extend(on_edge.iter().map(|&(_, v)| v));
        }
        // A vertex near a corner can be within the tolerance of *both* edges
        // meeting there, and would then be spliced into the loop twice. Once is
        // enough to make it a corner of the fan -- twice makes two triangles
        // that lie on top of each other, and an edge used twice in the same
        // direction is exactly as non-manifold as an edge used once.
        let mut seen = loop_.clone();
        seen.sort_unstable();
        if seen.windows(2).any(|w| w[0] == w[1]) {
            let mut kept: Vec<u32> = Vec::with_capacity(loop_.len());
            for &v in &loop_ {
                if !kept.contains(&v) {
                    kept.push(v);
                }
            }
            loop_ = kept;
        }
        if loop_.len() == 3 {
            indices.push(*tri);
            tags.push(tag);
            continue;
        }
        fan_from_centre(pos, tri, &loop_, tag, &mut positions, &mut indices, &mut tags);
    }

    Mesh { positions, indices, tags }
}

/// Every vertex of the mesh lying strictly inside edge `e` of `tri`, in order
/// along the edge. Written into `out` rather than returned so the walk over a
/// large mesh allocates nothing per triangle.
fn on_edge_vertices(
    pos: &[Vec3],
    grid: &HashMap<Cell, Vec<u32>>,
    size: f64,
    tol: f64,
    tri: &[u32; 3],
    e: usize,
    out: &mut Vec<(f64, u32)>,
) {
    out.clear();
    let (ia, ib) = (tri[e], tri[(e + 1) % 3]);
    let (pa, pb) = (pos[ia as usize], pos[ib as usize]);
    let ab = pb - pa;
    let len2 = ab.dot(ab);
    if len2 <= tol * tol {
        return;
    }
    let margin = tol / len2.sqrt();

    let lo = pa.min(pb) - Vec3::splat(tol);
    let hi = pa.max(pb) + Vec3::splat(tol);
    let (c0, c1) = (cell_of(lo, size), cell_of(hi, size));
    for cx in c0.0..=c1.0 {
        for cy in c0.1..=c1.1 {
            for cz in c0.2..=c1.2 {
                let Some(list) = grid.get(&(cx, cy, cz)) else { continue };
                for &v in list {
                    if v == tri[0] || v == tri[1] || v == tri[2] {
                        continue;
                    }
                    let d = pos[v as usize] - pa;
                    let s = d.dot(ab) / len2;
                    if s <= margin || s >= 1.0 - margin {
                        continue;
                    }
                    if (d - ab * s).length() > tol {
                        continue;
                    }
                    out.push((s, v));
                }
            }
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    // The same physical point can be present twice over -- the grid is searched
    // by cell, and a vertex sitting exactly on a cell boundary is listed in
    // both. Two boundary vertices at the same place would make a zero-length
    // edge, and the ear clipper below would have to cope with it.
    out.dedup_by(|a, b| (a.0 - b.0).abs() <= f64::EPSILON || a.1 == b.1);
}

/// Triangulate a triangle's boundary once its edges have been subdivided, by
/// fanning it from a new vertex at the triangle's centre.
///
/// The obvious triangulations both fail here. A fan from one of the corners
/// leaves every split point on the two edges meeting at that corner sitting in
/// the interior of an emitted edge -- the T-junction is not removed, only
/// moved. An ear clipper stalls: a boundary that is a triangle's own sides is
/// convex but full of collinear triples, and on a boolean's sliver triangles
/// *every* triple comes out collinear to within the tolerance, so it gives up
/// and drops the face, which tears a hole in the surface. Both were measured
/// doing exactly that before this was written.
///
/// The centre point is inside the triangle by construction, a third of the
/// height away from each side, so every triangle of the fan has real area and
/// every split point is a corner of two of them. It costs one vertex per
/// subdivided face, and `retriangulate_flat_regions` -- which runs immediately
/// after and rebuilds each flat region from its boundary alone -- drops them
/// again.
fn fan_from_centre(
    pos: &[Vec3],
    tri: &[u32; 3],
    boundary: &[u32],
    tag: u32,
    positions: &mut Vec<Vec3>,
    indices: &mut Vec<[u32; 3]>,
    tags: &mut Vec<u32>,
) {
    let centre = (pos[tri[0] as usize] + pos[tri[1] as usize] + pos[tri[2] as usize]) / 3.0;
    positions.push(centre);
    let c = (positions.len() - 1) as u32;
    for i in 0..boundary.len() {
        let (a, b) = (boundary[i], boundary[(i + 1) % boundary.len()]);
        indices.push([c, a, b]);
        tags.push(tag);
    }
}

/// Weld, cancel coincident opposite faces, eliminate T-junctions, and rebuild
/// each flat region's interior triangulation. Applied to every boolean result so
/// nested booleans always get clean, and reasonably sized, input.
pub fn heal(mesh: &Mesh) -> Mesh {
    let m = weld_tolerant(mesh, WELD_TOL);
    let m = drop_slivers(m, WELD_TOL);
    let m = cancel_opposite_faces(m);
    let healed = split_t_junctions(m, WELD_TOL);

    // Rebuilding each flat region deliberately straightens its boundary, dropping
    // the collinear vertices the pass above inserted -- an ear clipper stalls on
    // those. The neighbouring faces still have their own corners there, so a
    // second T-junction pass puts exactly the same splits back, this time into
    // far fewer and larger triangles.
    //
    // Retriangulation is the most intricate step here and the one most exposed to
    // geometry nobody anticipated, so its output only stands if it is at least as
    // sound as the input it replaced. A valid but bulky mesh always beats a slim
    // broken one.
    // Compacted before the second T-junction pass, and not just at the end:
    // retriangulating orphans every vertex that was interior to a flat region,
    // and those orphans sit *on* the large new triangles that replaced them.
    // Left in `positions` they would all be found as on-edge vertices and split
    // straight back out again.
    let simplified = compact(crate::planar::retriangulate_flat_regions(&healed));
    let simplified = split_t_junctions(simplified, WELD_TOL);
    if simplified.triangle_count() < healed.triangle_count()
        && (simplified.manifold_issue().is_none() || healed.manifold_issue().is_some())
    {
        return compact(simplified);
    }
    compact(healed)
}

/// Drop positions no triangle references (cancelling faces can orphan some).
fn compact(mesh: Mesh) -> Mesh {
    let mut remap = vec![u32::MAX; mesh.positions.len()];
    let mut positions = Vec::with_capacity(mesh.positions.len());
    let mut indices = Vec::with_capacity(mesh.indices.len());
    for t in &mesh.indices {
        let mut out = [0u32; 3];
        for k in 0..3 {
            let old = t[k] as usize;
            if remap[old] == u32::MAX {
                positions.push(mesh.positions[old]);
                remap[old] = (positions.len() - 1) as u32;
            }
            out[k] = remap[old];
        }
        indices.push(out);
    }
    Mesh { positions, indices, tags: mesh.tags.clone() }
}
