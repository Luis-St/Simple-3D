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

/// Merge the ends of every edge shorter than `limit`, and drop the triangles
/// that collapse to a line as a result.
///
/// The weld above merges points that are within a tolerance of *each other*;
/// this deals with the pair that ends up just outside it. A chain of booleans
/// puts two copies of the same physical point a few microns apart -- far enough
/// to survive the weld, close enough that the triangle between them is a
/// slither with no surface -- and there is no tolerance that separates that
/// case from a real feature by distance alone, because the two are the same
/// distance apart.
///
/// Collapsing, rather than deleting. A deleted sliver leaves its neighbours'
/// edges with nothing on the other side, which is a hole where there was a
/// seam; collapsing one end onto the other retargets those neighbours instead,
/// and the surface stays closed. Nothing moves further than `limit`.
fn collapse_short_edges(mesh: Mesh, limit: f64) -> Mesh {
    let mut union: Vec<u32> = (0..mesh.positions.len() as u32).collect();
    fn root(union: &mut [u32], mut i: u32) -> u32 {
        while union[i as usize] != i {
            union[i as usize] = union[union[i as usize] as usize];
            i = union[i as usize];
        }
        i
    }
    for t in &mesh.indices {
        for e in 0..3 {
            let (a, b) = (root(&mut union, t[e]), root(&mut union, t[(e + 1) % 3]));
            if a == b {
                continue;
            }
            if (mesh.positions[a as usize] - mesh.positions[b as usize]).length() < limit {
                // The lower index survives, so the result does not depend on
                // the order the triangles happen to be in.
                let (keep, gone) = if a < b { (a, b) } else { (b, a) };
                union[gone as usize] = keep;
            }
        }
    }
    let mut indices = Vec::with_capacity(mesh.indices.len());
    let mut tags = Vec::with_capacity(mesh.indices.len());
    for (i, t) in mesh.indices.iter().enumerate() {
        let t = [root(&mut union, t[0]), root(&mut union, t[1]), root(&mut union, t[2])];
        if t[0] != t[1] && t[1] != t[2] && t[0] != t[2] {
            indices.push(t);
            tags.push(mesh.tag(i));
        }
    }
    Mesh { positions: mesh.positions, indices, tags }
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
    let mut on_edge: [Vec<OnEdge>; 3] = [Vec::new(), Vec::new(), Vec::new()];

    for (i, tri) in mesh.indices.iter().enumerate() {
        let tag = mesh.tag(i);
        for (e, out) in on_edge.iter_mut().enumerate() {
            on_edge_vertices(pos, &grid, size, tol, tri, e, out);
        }
        loop_.clear();
        for e in 0..3 {
            loop_.push(tri[e]);
            loop_.extend(on_edge[e].iter().map(|v| v.vertex));
        }
        if loop_.len() == 3 {
            indices.push(*tri);
            tags.push(tag);
            continue;
        }
        let mut deduped = loop_.clone();
        deduped.sort_unstable();
        deduped.dedup();
        if deduped.len() == loop_.len() {
            fan_from_centre(pos, tri, &loop_, tag, &mut positions, &mut indices, &mut tags);
        } else {
            for piece in split_pinched_loops(&loop_) {
                fan_loop_from_own_centre(pos, &piece, tag, &mut positions, &mut indices, &mut tags);
            }
        }
    }

    Mesh { positions, indices, tags }
}

/// A mesh vertex found lying on one edge of a triangle: how far along that edge
/// it sits, and which vertex it is.
#[derive(Clone, Copy, Debug)]
struct OnEdge {
    along: f64,
    vertex: u32,
}

/// Every vertex of the mesh lying strictly inside edge `e` of `tri`, in order
/// along the edge. Written into `out` rather than returned so the walk over a
/// large mesh allocates nothing per triangle.
/// Split a triangle's boundary into simple loops wherever it touches the same
/// vertex twice.
///
/// A vertex is spliced into the boundary of *every* edge it lies on, and near a
/// sharp corner it can genuinely lie on both edges meeting there. The BSP
/// produces needles routinely -- two long, nearly parallel sides and a very
/// short end, 2.3 mm long and 5 microns wide in the case that prompted this --
/// and around one of those a vertex sitting exactly on one long side is inside
/// the `tol` band of the other as well.
///
/// Removing one of the two occurrences is what the pass used to do, and it is
/// the reason a boolean of two finely tessellated operands came out
/// non-manifold. The decision is made per triangle, but an edge is shared with a
/// neighbour that has no reason to make the same one: whichever occurrence is
/// dropped, the triangle across that edge still splits there, and the two sides
/// no longer agree. It cannot be repaired by running the pass again either --
/// each run manufactures a fresh disagreement somewhere else, which is why
/// repeating it diverged instead of converging.
///
/// Keeping both occurrences makes every triangle agree with its neighbours,
/// because whether a vertex lies on a segment depends on the segment alone. What
/// it costs is a boundary that is pinched at that vertex, and a pinched loop
/// cannot be fanned as one polygon -- the two triangles either side of the pinch
/// would share an edge in the same direction. So it is cut into simple loops
/// here, and each is fanned separately.
///
/// Every vertex of the boundary lies on the original triangle's own sides, so
/// each loop is convex and its own centroid is strictly inside it -- which is
/// what makes fanning each piece from its own centre sound.
fn split_pinched_loops(boundary: &[u32]) -> Vec<Vec<u32>> {
    let mut loops: Vec<Vec<u32>> = Vec::new();
    let mut stack: Vec<u32> = Vec::with_capacity(boundary.len());
    for &v in boundary {
        if let Some(at) = stack.iter().position(|&w| w == v) {
            // Everything since the last visit to `v` closes a loop of its own.
            let piece: Vec<u32> = stack[at..].to_vec();
            if piece.len() >= 3 {
                loops.push(piece);
            }
            stack.truncate(at);
        }
        stack.push(v);
    }
    if stack.len() >= 3 {
        loops.push(stack);
    }
    loops
}

fn on_edge_vertices(
    pos: &[Vec3],
    grid: &HashMap<Cell, Vec<u32>>,
    size: f64,
    tol: f64,
    tri: &[u32; 3],
    e: usize,
    out: &mut Vec<OnEdge>,
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
                    out.push(OnEdge { along: s, vertex: v });
                }
            }
        }
    }
    out.sort_by(|a, b| a.along.total_cmp(&b.along));
    // The same physical point can be present twice over -- the grid is searched
    // by cell, and a vertex sitting exactly on a cell boundary is listed in
    // both. Two boundary vertices at the same place would make a zero-length
    // edge, and the ear clipper below would have to cope with it.
    out.dedup_by(|a, b| (a.along - b.along).abs() <= f64::EPSILON || a.vertex == b.vertex);
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

/// Fan a loop from its own centroid, for the pieces a pinched boundary is cut
/// into. Unlike [`fan_from_centre`] there is no original triangle to take the
/// centre from -- the piece is only part of one -- but the piece is convex, so
/// the average of its own vertices is inside it.
fn fan_loop_from_own_centre(
    pos: &[Vec3],
    loop_: &[u32],
    tag: u32,
    positions: &mut Vec<Vec3>,
    indices: &mut Vec<[u32; 3]>,
    tags: &mut Vec<u32>,
) {
    if loop_.len() < 3 {
        return;
    }
    let mut centre = Vec3::ZERO;
    for &v in loop_ {
        centre = centre + pos[v as usize];
    }
    let centre = centre / loop_.len() as f64;
    positions.push(centre);
    let c = (positions.len() - 1) as u32;
    for i in 0..loop_.len() {
        let (a, b) = (loop_[i], loop_[(i + 1) % loop_.len()]);
        indices.push([c, a, b]);
        tags.push(tag);
    }
}

/// Weld, cancel coincident opposite faces, eliminate T-junctions, and rebuild
/// each flat region's interior triangulation. Applied to every boolean result so
/// nested booleans always get clean, and reasonably sized, input.
///
/// A second and a third attempt at a coarser tolerance, when the first leaves
/// the mesh broken. `WELD_TOL` is sized for the ~1e-12mm disagreement between
/// two evaluations of the same physical point, and that is the right size for
/// one boolean; nine of them chained -- each one's output the next one's input
/// -- push two copies of a point as far as 3e-6mm apart, and a weld that leaves
/// those as two points leaves a seam no amount of splitting can close. Which
/// side of the tolerance such a pair lands on comes down to the last bits of a
/// sine, so the same commit was clean on Linux and not on Windows.
///
/// The coarser attempts run from the original mesh rather than patching the
/// first attempt's output: a tolerance is a decision made at the weld, and
/// every step after it inherits that decision. They cost nothing in the
/// ordinary case, which stops at the first attempt, and a tenth of a micron is
/// still two orders of magnitude below anything a printer resolves.
pub fn heal(mesh: &Mesh) -> Mesh {
    let best = heal_at(mesh, WELD_TOL);
    if best.manifold_issue().is_none() {
        return best;
    }
    for factor in [10.0, 100.0, 1000.0] {
        let again = heal_at(mesh, WELD_TOL * factor);
        if again.manifold_issue().is_none() {
            return again;
        }
    }
    best
}

/// One attempt at healing, at one tolerance.
fn heal_at(mesh: &Mesh, tol: f64) -> Mesh {
    let m = weld_tolerant(mesh, tol);
    let m = collapse_short_edges(m, tol * 4.0);
    let m = drop_slivers(m, tol);
    let m = cancel_opposite_faces(m);
    let healed = split_t_junctions(m, tol);

    // Rebuilding each flat region deliberately straightens its boundary,
    // dropping the collinear vertices the pass above inserted -- an ear clipper
    // stalls on those. The neighbouring faces still have their own corners
    // there, so a second T-junction pass puts exactly the same splits back,
    // this time into far fewer and larger triangles.
    //
    // Compacted before that second pass, and not just at the end:
    // retriangulating orphans every vertex that was interior to a flat region,
    // and those orphans sit *on* the large new triangles that replaced them.
    // Left in `positions` they would all be found as on-edge vertices and split
    // straight back out again.
    let simplified = split_t_junctions(compact(crate::planar::retriangulate_flat_regions(&healed)), tol);
    cap_boundary_loops(compact(sounder_of(healed, simplified)))
}

/// The largest hole this is willing to put a lid on, as a fraction of the
/// model's own extent. A boolean of two closed solids has a closed result, so
/// every boundary loop in one is a defect; but a defect the size of the model
/// is a wrong answer, not a missing lid, and covering it over would hide that
/// where reporting the node as non-manifold does not.
const CAP_SPAN: f64 = 0.01;

/// Close what is left open, where what is left open is a hole rather than a
/// wrong answer.
///
/// The passes above all fix a surface that is *there* and mis-shared: welded,
/// collapsed, split at its T-junctions. None of them can do anything about a
/// triangle nothing ever emitted, and that is what the boolean's remaining
/// failures are. Every one of them, measured across every ordered pair of the
/// five primitives through all three operations, is a single closed loop of
/// three or four vertices spanning at most 0.38 mm on a 40 mm body: two faces
/// meeting at a grazing angle, one of them keeping a corner that lies within
/// `csg_bsp::EPSILON` of the plane that should have trimmed it, and the sliver
/// between that corner and where the other body's surface really comes down
/// belonging to neither operand. There is no T-junction there to split and no
/// pair of vertices close enough to weld -- the corners are microns to tenths
/// of a millimetre apart, which is a real distance -- so nothing here could
/// close it.
///
/// A closed loop, though, already says what the missing surface is: the loop
/// *is* the hole's boundary, and the lid that fills it is the only surface it
/// can bound. Fanning it is exact for the three-vertex case and within the
/// loop's own diameter of exact for the rest, which is to say within the error
/// that opened the hole in the first place.
///
/// Only simple closed loops, only loops smaller than `CAP_SPAN` of the model,
/// and only when the result is actually sounder than what went in.
fn cap_boundary_loops(mesh: Mesh) -> Mesh {
    use std::collections::{BTreeMap, BTreeSet};
    let Some((lo, hi)) = mesh.bounds() else { return mesh };
    let span = (hi - lo).length() * CAP_SPAN;

    let mut count: BTreeMap<(u32, u32), i32> = BTreeMap::new();
    for t in &mesh.indices {
        for k in 0..3 {
            *count.entry((t[k], t[(k + 1) % 3])).or_insert(0) += 1;
        }
    }
    // One outgoing boundary edge per vertex, or none: a vertex where two holes
    // meet does not say which loop continues through it, and guessing is how a
    // repair invents surface.
    let mut next: BTreeMap<u32, u32> = BTreeMap::new();
    let mut ambiguous: BTreeSet<u32> = BTreeSet::new();
    for (&(x, y), &c) in &count {
        if c > count.get(&(y, x)).copied().unwrap_or(0) && next.insert(x, y).is_some() {
            ambiguous.insert(x);
        }
    }
    if next.is_empty() {
        return mesh;
    }

    let mut capped = mesh.clone();
    let mut seen: BTreeSet<u32> = BTreeSet::new();
    for &start in next.keys() {
        if seen.contains(&start) {
            continue;
        }
        // Walk the chain of boundary edges from here. It is a loop worth
        // capping only if it comes back to where it started without meeting a
        // vertex two holes share.
        let mut loop_verts: Vec<u32> = Vec::new();
        let mut cur = start;
        let closed = loop {
            if ambiguous.contains(&cur) {
                break false;
            }
            if !seen.insert(cur) {
                break false;
            }
            loop_verts.push(cur);
            match next.get(&cur) {
                Some(&nx) => cur = nx,
                None => break false,
            }
            if cur == start {
                break true;
            }
        };
        if !closed || loop_verts.len() < 3 {
            continue;
        }
        let mut blo = mesh.positions[loop_verts[0] as usize];
        let mut bhi = blo;
        for &v in &loop_verts {
            blo = blo.min(mesh.positions[v as usize]);
            bhi = bhi.max(mesh.positions[v as usize]);
        }
        if (bhi - blo).length() > span {
            continue;
        }
        // A fan from one corner also lays down the diagonals from it, and a
        // diagonal that is already an edge of the mesh would be used a third
        // time -- which is how a lid over a quad whose corners are joined
        // across turns one defect into another. Fan from a corner whose
        // diagonals are new, and leave the loop alone if no corner has any.
        let n = loop_verts.len();
        let free = |a: u32, b: u32| !count.contains_key(&(a, b)) && !count.contains_key(&(b, a));
        let Some(apex) = (0..n).find(|&k| (2..n - 1).all(|i| free(loop_verts[k], loop_verts[(k + i) % n]))) else {
            continue;
        };
        // Wound against the loop: an edge the mesh used as `a -> b` is missing
        // its `b -> a`, so the lid has to run the other way round.
        let tag = capped.tags.first().copied().unwrap_or(0);
        for i in 1..n - 1 {
            capped.indices.push([loop_verts[apex], loop_verts[(apex + i + 1) % n], loop_verts[(apex + i) % n]]);
            capped.tags.push(tag);
        }
    }
    sounder_of(mesh, capped)
}

/// Whichever of the two is fit to be returned: a manifold mesh in preference to
/// a broken one, and the smaller of the two when there is nothing to choose
/// between them on that count.
///
/// Never the broken one while a whole mesh is on the table. The rule this
/// replaces kept the *first* candidate unless the second was strictly smaller,
/// which quietly let a broken result through whenever both were broken -- and
/// that is exactly the position a boolean that has gone marginally differently
/// on another platform puts this in.
fn sounder_of(healed: Mesh, simplified: Mesh) -> Mesh {
    match (healed.manifold_issue().is_none(), simplified.manifold_issue().is_none()) {
        (false, true) => simplified,
        (true, false) => healed,
        _ if simplified.triangle_count() < healed.triangle_count() => simplified,
        _ => healed,
    }
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

#[cfg(test)]
mod repair_tests {
    use super::*;

    fn is_simple(piece: &[u32]) -> bool {
        let mut seen = piece.to_vec();
        seen.sort_unstable();
        let before = seen.len();
        seen.dedup();
        seen.len() == before
    }

    /// A boundary pinched in the middle is cut into two loops, and nothing on it
    /// is lost. This is the shape the pass used to mishandle: a vertex spliced
    /// into two of a triangle's edges at once, which is what happens around the
    /// needle triangles a BSP produces.
    #[test]
    fn a_pinched_boundary_is_cut_into_two_simple_loops() {
        let boundary = [0, 1, 2, 3, 4, 2, 5, 6];
        let loops = split_pinched_loops(&boundary);
        assert_eq!(loops.len(), 2, "got {loops:?}");
        for piece in &loops {
            assert!(is_simple(piece), "piece {piece:?} still visits a vertex twice");
            assert!(piece.len() >= 3, "piece {piece:?} encloses no area");
        }
        let mut covered: Vec<u32> = loops.iter().flatten().copied().collect();
        covered.sort_unstable();
        covered.dedup();
        assert_eq!(covered, vec![0, 1, 2, 3, 4, 5, 6], "a vertex of the boundary was lost");
    }

    /// The pinch that actually turns up: the repeated vertex sits a hair from a
    /// corner, on both of the edges meeting there. The piece it cuts off is that
    /// corner and nothing else -- two vertices, enclosing no area -- so it is
    /// dropped rather than emitted as a degenerate triangle, and the corner goes
    /// with it. That is the right answer geometrically: the vertex and the corner
    /// are within tolerance of each other's edges, so the wedge between them has
    /// no surface to contribute, and the neighbouring triangles still carry the
    /// corner. `a_dense_round_operand_meeting_a_plate_stays_manifold` is what
    /// checks that no hole is left behind by it.
    #[test]
    fn a_pinch_against_a_corner_drops_the_wedge_that_has_no_area() {
        let boundary = [0, 9, 1, 4, 5, 6, 7, 8, 9];
        let loops = split_pinched_loops(&boundary);
        assert_eq!(loops, vec![vec![9, 1, 4, 5, 6, 7, 8]]);
        assert!(is_simple(&loops[0]));
    }

    /// An ordinary boundary -- every vertex distinct -- is one loop, unchanged.
    #[test]
    fn an_unpinched_boundary_is_left_as_one_loop() {
        let boundary = [0, 1, 2, 3, 4];
        assert_eq!(split_pinched_loops(&boundary), vec![vec![0, 1, 2, 3, 4]]);
    }

    /// A union of two finely tessellated operands is a closed solid. At 224 and
    /// 256 segments this came out with holes and doubled edges before the pinch
    /// was handled; 144 is the smallest form of the same case that still runs in
    /// about a second.
    #[test]
    fn a_dense_round_operand_meeting_a_plate_stays_manifold() {
        let cap = crate::primitives::spherical_cap_mesh(20.0, 6.0, 144);
        let plate = crate::primitives::plate_mesh(40.0, 40.0, 4.0);
        let result = crate::evaluate_boolean(crate::BooleanOp::Union, &[cap, plate]);
        assert_eq!(result.manifold_issue(), None, "the union is not a closed solid");
    }
    /// A closed body with a small tetrahedral pocket beside it that is missing
    /// one of its faces: a three-vertex hole, and the lid that fills it is the
    /// face that was taken out.
    #[test]
    fn a_hole_small_enough_to_be_a_defect_is_given_its_lid() {
        let open = open_tetrahedron(0.1);
        assert!(open.manifold_issue().is_some(), "this test needs an open surface to start with");
        let capped = cap_boundary_loops(open.clone());
        assert_eq!(capped.manifold_issue(), None, "the hole was not closed");
        assert_eq!(capped.triangle_count(), open.triangle_count() + 1);
    }

    /// The same hole, at a size no lid is safe at. A boundary loop a tenth of
    /// the model across is a wrong answer rather than a missing triangle, and
    /// covering it over would hide that where reporting it does not.
    #[test]
    fn a_hole_the_size_of_the_model_is_left_open() {
        let open = open_tetrahedron(20.0);
        assert!(cap_boundary_loops(open).manifold_issue().is_some(), "a hole this size must not be filled");
    }

    /// A 100 mm closed box, and beside it a tetrahedron of the given size with
    /// its base missing. The box is there to be the model: what may be capped
    /// is judged against the size of what is being repaired, so a hole has to
    /// be small relative to *something*.
    fn open_tetrahedron(size: f64) -> Mesh {
        let mut mesh = crate::primitives::box_mesh(100.0, 100.0, 100.0);
        let at = Vec3::new(80.0, 0.0, 0.0);
        let p = [at, at + Vec3::new(size, 0.0, 0.0), at + Vec3::new(0.0, size, 0.0), at + Vec3::new(0.0, 0.0, size)];
        // Three of the four faces, wound outwards; the base (0, 2, 1) is left out.
        mesh.push_triangle(p[0], p[1], p[3]);
        mesh.push_triangle(p[1], p[2], p[3]);
        mesh.push_triangle(p[2], p[0], p[3]);
        weld_tolerant(&mesh, WELD_TOL)
    }
}
