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
//! trying to make the BSP produce matched splits: weld coincident vertices
//! with a real tolerance, then repeatedly split any triangle edge that has a
//! foreign vertex lying on its interior until no such vertex remains.

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

    let indices = mesh
        .indices
        .iter()
        .map(|t| [remap[t[0] as usize], remap[t[1] as usize], remap[t[2] as usize]])
        .filter(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2])
        .collect();
    Mesh { positions, indices }
}

/// Drop triangles thinner than `tol`, measured as the smallest distance from a
/// vertex to the opposite edge. Welding has already merged everything closer
/// together than `tol`, so such a triangle is geometrically indistinguishable
/// from a line segment: it contributes no surface, but its three edges are
/// counted by the manifold check and by slicers. They arise wherever a face
/// carrying collinear T-junction vertices gets fan-triangulated.
fn drop_slivers(mesh: Mesh, tol: f64) -> Mesh {
    let pos = &mesh.positions;
    let indices: Vec<[u32; 3]> = mesh
        .indices
        .iter()
        .copied()
        .filter(|t| {
            let (a, b, c) = (pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]);
            let twice_area = (b - a).cross(c - a).length();
            let longest = (b - a).length().max((c - b).length()).max((a - c).length());
            longest > tol && twice_area / longest > tol
        })
        .collect();
    Mesh { positions: mesh.positions, indices }
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
    let indices = mesh.indices.iter().enumerate().filter(|(i, _)| !dead[*i]).map(|(_, t)| *t).collect();
    Mesh { positions: mesh.positions, indices }
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

    let pos = &mesh.positions;
    let mut pending: Vec<[u32; 3]> = mesh.indices;
    let mut out: Vec<[u32; 3]> = Vec::with_capacity(pending.len());
    // Each split strictly consumes one on-edge vertex, so this terminates; the
    // budget only guards against a pathological input turning a preview into a
    // hang, in which case we emit what we have and the manifold check reports.
    let mut budget = 4_000_000usize;

    while let Some(tri) = pending.pop() {
        if budget == 0 {
            out.push(tri);
            continue;
        }
        budget -= 1;
        match find_on_edge_vertex(pos, &grid, size, tol, &tri) {
            Some((e, v)) => {
                let (a, b, c) = (tri[e], tri[(e + 1) % 3], tri[(e + 2) % 3]);
                pending.push([a, v, c]);
                pending.push([v, b, c]);
            }
            None => out.push(tri),
        }
    }

    Mesh { positions: mesh.positions, indices: out }
}

/// Find a vertex lying strictly inside one of the triangle's edges. Returns
/// the edge's index within the triangle and the offending vertex, choosing the
/// vertex nearest the edge's start so repeated splitting walks along the edge.
fn find_on_edge_vertex(
    pos: &[Vec3],
    grid: &HashMap<Cell, Vec<u32>>,
    size: f64,
    tol: f64,
    tri: &[u32; 3],
) -> Option<(usize, u32)> {
    for e in 0..3 {
        let (ia, ib) = (tri[e], tri[(e + 1) % 3]);
        let (pa, pb) = (pos[ia as usize], pos[ib as usize]);
        let ab = pb - pa;
        let len2 = ab.dot(ab);
        if len2 <= tol * tol {
            continue;
        }
        let len = len2.sqrt();
        let margin = tol / len;

        let lo = pa.min(pb) - Vec3::splat(tol);
        let hi = pa.max(pb) + Vec3::splat(tol);
        let (c0, c1) = (cell_of(lo, size), cell_of(hi, size));

        let mut best: Option<(f64, u32)> = None;
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
                        if best.map_or(true, |(bs, _)| s < bs) {
                            best = Some((s, v));
                        }
                    }
                }
            }
        }
        if let Some((_, v)) = best {
            return Some((e, v));
        }
    }
    None
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
    Mesh { positions, indices }
}
