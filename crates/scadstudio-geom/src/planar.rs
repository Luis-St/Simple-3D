//! Retriangulating the flat regions of a boolean result.
//!
//! A BSP boolean clips against *infinite* planes. Subtracting an 8 mm
//! 24-segment cylinder from a 40 x 20 mm plate therefore slices the plate's
//! entire top and bottom face along 24 lines that run right across it, not just
//! around the hole. The result is correct, watertight and manifold, but a plate
//! with one hole and one boss comes out at ~1500 triangles where ~300 describe
//! the same solid -- and every later boolean in the chain pays for those
//! triangles again.
//!
//! The fix is to throw the interior triangulation of each flat region away and
//! rebuild it from the region's own boundary. Two properties make that safe:
//!
//! * **The boundary is preserved exactly.** Every vertex on a region's boundary
//!   loops survives into the new triangulation, including the collinear ones
//!   [`crate::repair::split_t_junctions`] put there. Neighbouring faces on other
//!   planes are untouched and still meet this one vertex-for-vertex, so healing
//!   is not undone.
//! * **Only strictly-interior vertices are dropped**, and in a manifold mesh a
//!   vertex interior to a flat region belongs to no other face.
//!
//! A region left behind by an earlier boolean is routinely concave and has holes
//! in it (that is exactly what a drilled plate's top face is), so this needs a
//! real polygon-with-holes triangulator rather than a fan: holes are bridged
//! into the outer loop and the result is ear-clipped. Anything the pass cannot
//! make sense of -- a boundary that pinches at a vertex, a loop that will not
//! close, a triangulation that runs out of ears -- leaves that region's original
//! triangles alone rather than guessing.

use crate::mesh::Mesh;
use crate::vec3::Vec3;
use std::collections::BTreeMap;

/// Rebuild the triangulation of every flat region of `mesh` from its boundary.
/// Regions that cannot be interpreted keep their original triangles, so this
/// never fails -- at worst it changes nothing.
pub fn retriangulate_flat_regions(mesh: &Mesh) -> Mesh {
    let mut groups: BTreeMap<(i64, i64, i64, i64), Vec<usize>> = BTreeMap::new();
    let mut ungrouped: Vec<[u32; 3]> = Vec::new();
    for (i, t) in mesh.indices.iter().enumerate() {
        match plane_of(mesh, *t) {
            Some((normal, w)) => groups.entry(plane_key(normal, w)).or_default().push(i),
            // Degenerate: no usable plane. Left exactly as it was.
            None => ungrouped.push(*t),
        }
    }

    let mut out = ungrouped;
    for tris in groups.values() {
        let original = || tris.iter().map(|&i| mesh.indices[i]);
        if tris.len() < 3 {
            // A one- or two-triangle region has nothing to gain and no interior
            // vertex to drop.
            out.extend(original());
            continue;
        }
        let normal = plane_of(mesh, mesh.indices[tris[0]]).unwrap().0;
        match boundary_loops(mesh, tris).and_then(|loops| triangulate_region(&mesh.positions, normal, loops)) {
            Some(rebuilt) if rebuilt.len() <= tris.len() => out.extend(rebuilt),
            // Either the region was uninterpretable, or rebuilding it produced
            // *more* triangles than it started with -- in which case there was
            // nothing to win and the original is the safer answer.
            _ => out.extend(original()),
        }
    }
    Mesh { positions: mesh.positions.clone(), indices: out }
}

fn plane_of(mesh: &Mesh, t: [u32; 3]) -> Option<(Vec3, f64)> {
    let (a, b, c) = (mesh.positions[t[0] as usize], mesh.positions[t[1] as usize], mesh.positions[t[2] as usize]);
    let n = (b - a).cross(c - a);
    if n.length() < 1e-12 {
        return None;
    }
    let n = n.normalized();
    Some((n, n.dot(a)))
}

/// Quantised so two triangles of the same physical face land in the same group
/// despite last-bit differences in their computed normals. Deliberately
/// sign-sensitive: two faces back to back are not one region.
fn plane_key(normal: Vec3, w: f64) -> (i64, i64, i64, i64) {
    let s = 1_000_000.0;
    (
        (normal.x * s).round() as i64,
        (normal.y * s).round() as i64,
        (normal.z * s).round() as i64,
        (w * s).round() as i64,
    )
}

/// The region's boundary, as closed loops of vertex indices wound the same way
/// as the triangles that produced them.
///
/// An edge interior to the region is used once in each direction by the two
/// triangles sharing it; a boundary edge is used in one direction only. Returns
/// `None` if the region is not a clean set of simple loops -- a directed edge
/// used twice (the region overlaps itself), or a vertex with two outgoing
/// boundary edges (the boundary pinches there, and which way to turn is a guess).
fn boundary_loops(mesh: &Mesh, tris: &[usize]) -> Option<Vec<Vec<u32>>> {
    let mut used: BTreeMap<(u32, u32), u32> = BTreeMap::new();
    for &ti in tris {
        let t = mesh.indices[ti];
        for k in 0..3 {
            let count = used.entry((t[k], t[(k + 1) % 3])).or_insert(0);
            *count += 1;
            if *count > 1 {
                return None;
            }
        }
    }

    let mut next: BTreeMap<u32, u32> = BTreeMap::new();
    for &(a, b) in used.keys() {
        if used.contains_key(&(b, a)) {
            continue; // interior edge
        }
        if next.insert(a, b).is_some() {
            return None; // the boundary pinches at `a`
        }
    }
    if next.is_empty() {
        return None; // a closed surface with no boundary is not a flat region
    }

    let total = next.len();
    let mut loops: Vec<Vec<u32>> = Vec::new();
    while let Some((&start, _)) = next.iter().next() {
        let mut loop_verts = Vec::new();
        let mut current = start;
        loop {
            loop_verts.push(current);
            current = next.remove(&current)?;
            if current == start {
                break;
            }
            if loop_verts.len() > total {
                return None;
            }
        }
        if loop_verts.len() < 3 {
            return None;
        }
        loops.push(loop_verts);
    }
    Some(loops)
}

/// An orthonormal pair spanning the plane, so the region can be triangulated in
/// 2D with `normal` as the outward direction: a loop wound counter-clockwise
/// about `normal` has positive area.
fn plane_basis(normal: Vec3) -> (Vec3, Vec3) {
    let seed = if normal.x.abs() < 0.9 { Vec3::new(1.0, 0.0, 0.0) } else { Vec3::new(0.0, 1.0, 0.0) };
    let u = seed.cross(normal).normalized();
    (u, normal.cross(u))
}

type Point = (f64, f64);

fn signed_area(points: &[Point]) -> f64 {
    let n = points.len();
    let mut sum = 0.0;
    for i in 0..n {
        let (a, b) = (points[i], points[(i + 1) % n]);
        sum += a.0 * b.1 - b.0 * a.1;
    }
    sum * 0.5
}

fn triangulate_region(positions: &[Vec3], normal: Vec3, loops: Vec<Vec<u32>>) -> Option<Vec<[u32; 3]>> {
    let (u, v) = plane_basis(normal);
    let flatten = |ids: &[u32]| -> Vec<Point> {
        ids.iter().map(|&i| (positions[i as usize].dot(u), positions[i as usize].dot(v))).collect()
    };

    // Wound with the surface normal, an outer boundary encloses positive area
    // and a hole encloses negative area.
    let mut outers: Vec<(Vec<u32>, Vec<Point>, f64)> = Vec::new();
    let mut holes: Vec<(Vec<u32>, Vec<Point>)> = Vec::new();
    for ids in loops {
        let (ids, points) = drop_collinear(&ids, &flatten(&ids))?;
        let area = signed_area(&points);
        if area.abs() < 1e-18 {
            return None; // a degenerate loop; not worth guessing at
        }
        if area > 0.0 {
            outers.push((ids, points, area));
        } else {
            holes.push((ids, points));
        }
    }
    if outers.is_empty() {
        return None;
    }

    // One plane can carry several separate islands, so each hole belongs to the
    // smallest outer loop that contains it.
    let mut assigned: Vec<Vec<usize>> = vec![Vec::new(); outers.len()];
    for (h, (_, points)) in holes.iter().enumerate() {
        let probe = points[0];
        let owner = outers
            .iter()
            .enumerate()
            .filter(|(_, (_, outer, _))| point_in_polygon(probe, outer))
            .min_by(|(_, a), (_, b)| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)?;
        assigned[owner].push(h);
    }

    let mut out = Vec::new();
    for (i, (ids, points, _)) in outers.iter().enumerate() {
        let (mut ids, mut points) = (ids.clone(), points.clone());
        // Outermost first: bridging a hole splices it into the outer loop, and
        // a later bridge has to be able to see the seam the earlier one left.
        let mut mine = assigned[i].clone();
        mine.sort_by(|&a, &b| {
            let key = |h: usize| holes[h].1.iter().fold(f64::MIN, |m: f64, p| m.max(p.0));
            key(b).partial_cmp(&key(a)).unwrap_or(std::cmp::Ordering::Equal)
        });
        for h in mine {
            bridge_hole(&mut ids, &mut points, &holes[h].0, &holes[h].1)?;
        }
        ear_clip(&ids, &points, &mut out)?;
    }
    Some(out)
}

/// Drop vertices that lie on the straight line between their neighbours, giving
/// the ear clipper a loop whose every vertex is a genuine corner.
///
/// A healed boundary is full of collinear vertices -- that is precisely what
/// [`crate::repair::split_t_junctions`] put there so this face's edges match the
/// neighbouring face's. Feeding them to an ear clipper is what makes it stall:
/// they are never valid ear apexes, so a run of them can be left as the final
/// three vertices with no ear to take. They are not lost by removing them here,
/// because `heal` runs the T-junction pass again afterwards and re-splits the
/// long edges this leaves at exactly the same points.
///
/// Returns `None` for a loop with fewer than three genuine corners: it encloses
/// no area, and guessing at it is worse than leaving the region alone.
fn drop_collinear(ids: &[u32], points: &[Point]) -> Option<(Vec<u32>, Vec<Point>)> {
    let n = ids.len();
    // Comparing against immediate neighbours is enough even for a run of three
    // or more collinear vertices: every vertex strictly inside such a run has
    // two neighbours on the same line, so one pass drops the whole run.
    let corner = |i: usize| {
        let (a, b, c) = (points[(i + n - 1) % n], points[i], points[(i + 1) % n]);
        let cross = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
        let span = ((c.0 - a.0).powi(2) + (c.1 - a.1).powi(2)).sqrt();
        span > 1e-12 && cross.abs() / span > 1e-9
    };
    let mut kept_ids = Vec::with_capacity(n);
    let mut kept_points = Vec::with_capacity(n);
    for i in 0..n {
        if corner(i) {
            kept_ids.push(ids[i]);
            kept_points.push(points[i]);
        }
    }
    if kept_ids.len() < 3 {
        return None;
    }
    Some((kept_ids, kept_points))
}

fn point_in_polygon(p: Point, polygon: &[Point]) -> bool {
    let n = polygon.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (polygon[i], polygon[(i + 1) % n]);
        if (a.1 > p.1) != (b.1 > p.1) {
            let x = a.0 + (p.1 - a.1) / (b.1 - a.1) * (b.0 - a.0);
            if x > p.0 {
                inside = !inside;
            }
        }
    }
    inside
}

/// Splice `hole` into `outer` along a bridge, turning a polygon with a hole into
/// a single (self-touching) loop that an ear clipper can eat.
///
/// The bridge runs from the hole's rightmost vertex to the nearest outer vertex
/// it can reach without crossing any edge of either loop. Both endpoints appear
/// twice in the result, which is what makes the seam infinitely thin and leaves
/// the enclosed area unchanged.
fn bridge_hole(outer: &mut Vec<u32>, outer_points: &mut Vec<Point>, hole: &[u32], hole_points: &[Point]) -> Option<()> {
    let start = (0..hole_points.len())
        .max_by(|&a, &b| hole_points[a].0.partial_cmp(&hole_points[b].0).unwrap_or(std::cmp::Ordering::Equal))?;
    let from = hole_points[start];

    let mut candidates: Vec<usize> = (0..outer_points.len()).collect();
    candidates.sort_by(|&a, &b| {
        let d = |i: usize| {
            let p = outer_points[i];
            (p.0 - from.0).powi(2) + (p.1 - from.1).powi(2)
        };
        d(a).partial_cmp(&d(b)).unwrap_or(std::cmp::Ordering::Equal)
    });

    let target = candidates.into_iter().find(|&c| {
        let to = outer_points[c];
        !crosses_any(from, to, outer_points) && !crosses_any(from, to, hole_points)
    })?;

    // outer[..=target] + hole from `start` all the way round + hole[start] + outer[target..]
    let mut ids = Vec::with_capacity(outer.len() + hole.len() + 2);
    let mut points = Vec::with_capacity(ids.capacity());
    for i in 0..=target {
        ids.push(outer[i]);
        points.push(outer_points[i]);
    }
    for k in 0..=hole.len() {
        let i = (start + k) % hole.len();
        ids.push(hole[i]);
        points.push(hole_points[i]);
    }
    for i in target..outer.len() {
        ids.push(outer[i]);
        points.push(outer_points[i]);
    }
    *outer = ids;
    *outer_points = points;
    Some(())
}

/// Whether the open segment `a`-`b` properly crosses any edge of `polygon`.
/// Touching at an endpoint does not count: a bridge is *meant* to land on a
/// vertex of both loops.
fn crosses_any(a: Point, b: Point, polygon: &[Point]) -> bool {
    let n = polygon.len();
    (0..n).any(|i| segments_properly_cross(a, b, polygon[i], polygon[(i + 1) % n]))
}

fn segments_properly_cross(a: Point, b: Point, c: Point, d: Point) -> bool {
    let side = |p: Point, q: Point, r: Point| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    const EPS: f64 = 1e-12;
    let (d1, d2, d3, d4) = (side(a, b, c), side(a, b, d), side(c, d, a), side(c, d, b));
    // Strict signs on both segments: shared endpoints and collinear overlaps
    // fall through as "not crossing", which is what a bridge needs.
    ((d1 > EPS && d2 < -EPS) || (d1 < -EPS && d2 > EPS)) && ((d3 > EPS && d4 < -EPS) || (d3 < -EPS && d4 > EPS))
}

/// Ear-clip a counter-clockwise loop, appending triangles to `out`.
///
/// Two rules beyond the textbook version, both about *not losing a vertex*.
///
/// The whole point of this pass is that the region's boundary comes out
/// unchanged, and a vertex that no emitted triangle mentions has silently left
/// the boundary -- reopening exactly the T-junction
/// [`crate::repair::split_t_junctions`] closed. So a zero-area ear (three
/// collinear vertices, which is what a healed boundary is full of) is never
/// clipped: its apex stays in the loop and gets used as a neighbour of some
/// other ear instead.
///
/// The exception is the seam a bridged hole leaves, where one vertex appears
/// twice in the loop. There a zero-area ear is exactly what should be removed,
/// and doing so loses nothing because the other copy still carries the vertex.
fn ear_clip(ids: &[u32], points: &[Point], out: &mut Vec<[u32; 3]>) -> Option<()> {
    let mut remaining: Vec<usize> = (0..ids.len()).collect();
    let mut guard = ids.len() * ids.len() + 16;
    while remaining.len() > 3 {
        guard = guard.checked_sub(1)?;
        let n = remaining.len();
        let mut clipped = None;
        let mut seam = None;
        for i in 0..n {
            let (ia, ib, ic) = (remaining[(i + n - 1) % n], remaining[i], remaining[(i + 1) % n]);
            let (a, b, c) = (points[ia], points[ib], points[ic]);
            let turn = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
            if turn.abs() <= 1e-12 {
                if seam.is_none() && duplicated(ids, &remaining, ib) {
                    seam = Some(i);
                }
                continue;
            }
            if turn < 0.0 {
                continue; // reflex
            }
            if remaining.iter().any(|&j| j != ia && j != ib && j != ic && strictly_inside(points[j], a, b, c)) {
                continue;
            }
            clipped = Some((i, [ids[ia], ids[ib], ids[ic]]));
            break;
        }
        match clipped {
            Some((i, tri)) => {
                out.push(tri);
                remaining.remove(i);
            }
            // No real ear left. Unpicking a bridge seam may expose one; if there
            // is no seam either, this loop is beyond us and the caller keeps the
            // region's original triangles.
            None => {
                remaining.remove(seam?);
            }
        }
    }
    if remaining.len() == 3 {
        let (a, b, c) = (points[remaining[0]], points[remaining[1]], points[remaining[2]]);
        if ((b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)).abs() <= 1e-12 {
            // Three collinear vertices left over: emitting them would be a
            // sliver, dropping them would lose a boundary vertex.
            return None;
        }
        out.push([ids[remaining[0]], ids[remaining[1]], ids[remaining[2]]]);
    }
    Some(())
}

/// Whether the vertex at loop slot `slot` still has another slot in the loop
/// naming the same mesh vertex -- the signature of a bridge seam.
fn duplicated(ids: &[u32], remaining: &[usize], slot: usize) -> bool {
    remaining.iter().filter(|&&s| ids[s] == ids[slot]).count() > 1
}

fn strictly_inside(p: Point, a: Point, b: Point, c: Point) -> bool {
    let side = |q: Point, r: Point| (r.0 - q.0) * (p.1 - q.1) - (r.1 - q.1) * (p.0 - q.0);
    // The loop is counter-clockwise, so an interior point is left of all three
    // edges. Points *on* an edge do not block: a bridge seam puts them there by
    // construction.
    side(a, b) > 1e-12 && side(b, c) > 1e-12 && side(c, a) > 1e-12
}
