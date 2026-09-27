//! Closing boundary loops a repair has left open.

use crate::mesh::{FastMap, Mesh};

/// The largest hole to cap, as a fraction of the model's extent. A bigger hole is a wrong answer
/// that should be reported, not covered.
pub(crate) const CAP_SPAN: f64 = 0.01;

/// Close holes left open, where they are holes rather than wrong answers.
///
/// The other passes fix surface that exists but is mis-shared. The boolean's remaining failures
/// are missing triangles: small closed loops (at most 0.38 mm on a 40 mm body across all primitive
/// pairs and operations) from grazing faces within `csg_bsp::EPSILON`, with nothing to weld or
/// split. A closed loop defines its own lid, and fanning it is exact for three vertices and within
/// the loop's diameter otherwise.
///
/// Only simple closed loops below `CAP_SPAN`, and only when the result is sounder.
pub(crate) fn cap_boundary_loops(mesh: Mesh) -> Mesh {
    use std::collections::{BTreeMap, BTreeSet};
    let Some((lo, hi)) = mesh.bounds() else { return mesh };
    let span = (hi - lo).length() * CAP_SPAN;

    // Hashed, since it is only looked up and this runs over every edge several times per boolean.
    let mut count: FastMap<(u32, u32), i32> = FastMap::default();
    count.reserve(mesh.indices.len() * 3);
    for t in &mesh.indices {
        for k in 0..3 {
            *count.entry((t[k], t[(k + 1) % 3])).or_insert(0) += 1;
        }
    }
    // One outgoing boundary edge per vertex or none: where two holes meet, guessing would invent surface.
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
        // Worth capping only if the chain returns to its start without meeting a shared vertex.
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
        // Fan from a corner whose diagonals are not already mesh edges, which would be used a third
        // time; skip the loop if there is none.
        let n = loop_verts.len();
        let free = |a: u32, b: u32| !count.contains_key(&(a, b)) && !count.contains_key(&(b, a));
        let Some(apex) = (0..n).find(|&k| (2..n - 1).all(|i| free(loop_verts[k], loop_verts[(k + i) % n]))) else {
            continue;
        };
        // Wound against the loop: the mesh's `a -> b` edge is missing its `b -> a`.
        let tag = capped.tags.first().copied().unwrap_or(0);
        for i in 1..n - 1 {
            capped.indices.push([loop_verts[apex], loop_verts[(apex + i + 1) % n], loop_verts[(apex + i) % n]]);
            capped.tags.push(tag);
        }
    }
    sounder_of(mesh, capped)
}

/// The fitter of the two: manifold over broken, less broken over more, then fewer triangles.
///
/// The old rule kept the first unless the second was smaller, letting a broken result through
/// when both were. Fewer triangles is not the tie-break between broken ones either: on a dense
/// import the smaller had 74 open edges against 8 that `cap_boundary_loops` could have closed.
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

/// Directed edges of the welded mesh without their reverse: zero exactly when
/// `Mesh::manifold_issue` reports nothing.
pub(crate) fn defect_count(mesh: &Mesh) -> usize {
    let welded = mesh.weld();
    let mut directed: FastMap<(u32, u32), u32> = FastMap::default();
    for tri in &welded.indices {
        for i in 0..3 {
            *directed.entry((tri[i], tri[(i + 1) % 3])).or_insert(0) += 1;
        }
    }
    directed.iter().filter(|(&(a, b), &count)| directed.get(&(b, a)).copied().unwrap_or(0) != count).count()
}

/// Replace every needle (a triangle with collinear corners) and its neighbour across the long side
/// by two real triangles.
///
/// `cap_boundary_loops` produces these over collinear slits, and the exporter rejects zero-area
/// triangles. Splitting the neighbour at the middle corner is exact. A lid over a longer collinear
/// loop is a fan of needles, so this repeats until a pass finds nothing.
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
    let height = |mesh: &Mesh, a: u32, b: u32, p: u32| {
        let (a, b, p) = (mesh.positions[a as usize], mesh.positions[b as usize], mesh.positions[p as usize]);
        let base = (b - a).length();
        if base == 0.0 {
            return 0.0;
        }
        (b - a).cross(p - a).length() / base
    };
    // The long side runs from corner `k` to `k + 1`; the remaining corner lies on it.
    let needle = |mesh: &Mesh, t: [u32; 3]| {
        let lengths: [f64; 3] =
            std::array::from_fn(|k| (mesh.positions[t[(k + 1) % 3] as usize] - mesh.positions[t[k] as usize]).length());
        let k = (0..3).max_by(|&x, &y| lengths[x].total_cmp(&lengths[y])).expect("three sides");
        let (a, c, b) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
        (height(mesh, a, c, b) <= tol).then_some((a, c, b))
    };
    let needles: Vec<(usize, (u32, u32, u32))> =
        mesh.indices.iter().enumerate().filter_map(|(i, &t)| Some((i, needle(&mesh, t)?))).collect();
    if needles.is_empty() {
        return (mesh, false);
    }
    // Only the edges across needles' long sides are indexed: a map of every edge dominated the cost.
    // The last triangle using an edge wins, as before.
    let mut by_edge: FastMap<(u32, u32), usize> = needles.iter().map(|&(_, (a, c, _))| ((c, a), usize::MAX)).collect();
    for (i, t) in mesh.indices.iter().enumerate() {
        for k in 0..3 {
            if let Some(slot) = by_edge.get_mut(&(t[k], t[(k + 1) % 3])) {
                *slot = i;
            }
        }
    }
    let mut done = vec![false; mesh.indices.len()];
    let mut changed = false;
    // Found before any triangle changed, the state every one of them is read in.
    for (i, (a, c, b)) in needles {
        if done[i] {
            continue;
        }
        let Some(&j) = by_edge.get(&(c, a)).filter(|&&j| j != usize::MAX) else { continue };
        if done[j] || j == i {
            continue;
        }
        let u = mesh.indices[j];
        let Some(d) = u.iter().copied().find(|&v| v != a && v != c) else { continue };
        if height(&mesh, a, c, d) <= tol {
            continue;
        }
        let tag = mesh.tag(j);
        // The needle runs a -> c -> b and `u` runs c -> a -> d, with `b` between c and a: `u` split at `b`.
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
