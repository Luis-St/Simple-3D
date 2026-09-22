//! Closing boundary loops a repair has left open.

use crate::mesh::Mesh;

/// The largest hole this is willing to put a lid on, as a fraction of the
/// model's own extent. A boolean of two closed solids has a closed result, so
/// every boundary loop in one is a defect; but a defect the size of the model
/// is a wrong answer, not a missing lid, and covering it over would hide that
/// where reporting the node as non-manifold does not.
pub(crate) const CAP_SPAN: f64 = 0.01;

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
pub(crate) fn cap_boundary_loops(mesh: Mesh) -> Mesh {
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
/// a broken one, the less broken of two broken ones, and the smaller of the two
/// when there is nothing to choose between them on either count.
///
/// Never the broken one while a whole mesh is on the table. The rule this
/// replaces kept the *first* candidate unless the second was strictly smaller,
/// which quietly let a broken result through whenever both were broken -- and
/// that is exactly the position a boolean that has gone marginally differently
/// on another platform puts this in.
///
/// Between two broken ones, fewer triangles was the tie-break until a boolean
/// on a dense imported surface showed what that costs: the retriangulated
/// candidate is always the smaller, and it came out with 74 open edges where
/// the one it replaced had 8 -- small enough for `cap_boundary_loops` to close,
/// and thrown away before it could.
pub(crate) fn sounder_of(healed: Mesh, simplified: Mesh) -> Mesh {
    let (h, s) = (defect_count(&healed), defect_count(&simplified));
    match (h == 0, s == 0) {
        (false, true) => simplified,
        (true, false) => healed,
        (false, false) if s != h => {
            if s < h {
                simplified
            } else {
                healed
            }
        }
        _ if simplified.triangle_count() < healed.triangle_count() => simplified,
        _ => healed,
    }
}

/// How many directed edges of the welded mesh are not matched by their
/// reverse: zero exactly when `Mesh::manifold_issue` has nothing to report.
pub(crate) fn defect_count(mesh: &Mesh) -> usize {
    use std::collections::HashMap;
    let welded = mesh.weld();
    let mut directed: HashMap<(u32, u32), u32> = HashMap::new();
    for tri in &welded.indices {
        for i in 0..3 {
            *directed.entry((tri[i], tri[(i + 1) % 3])).or_insert(0) += 1;
        }
    }
    directed.iter().filter(|(&(a, b), &count)| directed.get(&(b, a)).copied().unwrap_or(0) != count).count()
}

/// Replace every needle -- a triangle whose three corners lie on one line --
/// and its neighbour across the needle's long side by two real triangles.
///
/// `cap_boundary_loops` lays exactly such a lid over a slit whose corners are
/// collinear: the loop is closed and the mesh manifold, but a triangle with no
/// area has no normal, and the exporter rightly refuses a file with one in it
/// -- the black of a coloured import came back from its boolean with two. The middle corner lies on the long
/// side, so splitting the neighbour there is exact, and the two edges the
/// needle shared with the rest of the mesh go to the two halves unchanged.
///
/// A lid over a collinear loop of more than three corners is a fan of needles,
/// each one's long side shared with the next, so this runs until a pass finds
/// nothing: every split gives the needle beside it a real neighbour.
pub(crate) fn split_needles(mut mesh: Mesh, tol: f64) -> Mesh {
    for _ in 0..16 {
        let (split, changed) = split_needles_once(mesh, tol);
        mesh = split;
        if !changed {
            break;
        }
    }
    mesh
}

fn split_needles_once(mut mesh: Mesh, tol: f64) -> (Mesh, bool) {
    use std::collections::HashMap;
    let mut by_edge: HashMap<(u32, u32), usize> = HashMap::new();
    for (i, t) in mesh.indices.iter().enumerate() {
        for k in 0..3 {
            by_edge.insert((t[k], t[(k + 1) % 3]), i);
        }
    }
    let height = |mesh: &Mesh, a: u32, b: u32, p: u32| {
        let (a, b, p) = (mesh.positions[a as usize], mesh.positions[b as usize], mesh.positions[p as usize]);
        let base = (b - a).length();
        if base == 0.0 {
            return 0.0;
        }
        (b - a).cross(p - a).length() / base
    };
    let mut done = vec![false; mesh.indices.len()];
    let mut changed = false;
    for i in 0..mesh.indices.len() {
        if done[i] {
            continue;
        }
        let t = mesh.indices[i];
        let lengths: [f64; 3] = std::array::from_fn(|k| {
            (mesh.positions[t[(k + 1) % 3] as usize] - mesh.positions[t[k] as usize]).length()
        });
        // The long side runs from corner `k` to corner `k + 1`; the corner
        // left over is the one lying on it.
        let k = (0..3).max_by(|&x, &y| lengths[x].total_cmp(&lengths[y])).expect("three sides");
        let (a, c, b) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
        if height(&mesh, a, c, b) > tol {
            continue;
        }
        let Some(&j) = by_edge.get(&(c, a)) else { continue };
        if done[j] || j == i {
            continue;
        }
        let u = mesh.indices[j];
        let Some(d) = u.iter().copied().find(|&v| v != a && v != c) else { continue };
        if height(&mesh, a, c, d) <= tol {
            continue;
        }
        let tag = mesh.tag(j);
        // The needle runs a -> c -> b and `u` runs c -> a -> d, with `b`
        // between c and a: `u` split at `b`, and the needle gone.
        mesh.indices[i] = [c, b, d];
        mesh.indices[j] = [b, a, d];
        if mesh.tags.len() == mesh.indices.len() {
            mesh.tags[i] = tag;
        }
        done[i] = true;
        done[j] = true;
        changed = true;
    }
    (mesh, changed)
}
