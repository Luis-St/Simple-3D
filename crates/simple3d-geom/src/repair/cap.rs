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
/// a broken one, and the smaller of the two when there is nothing to choose
/// between them on that count.
///
/// Never the broken one while a whole mesh is on the table. The rule this
/// replaces kept the *first* candidate unless the second was strictly smaller,
/// which quietly let a broken result through whenever both were broken -- and
/// that is exactly the position a boolean that has gone marginally differently
/// on another platform puts this in.
pub(crate) fn sounder_of(healed: Mesh, simplified: Mesh) -> Mesh {
    match (healed.manifold_issue().is_none(), simplified.manifold_issue().is_none()) {
        (false, true) => simplified,
        (true, false) => healed,
        _ if simplified.triangle_count() < healed.triangle_count() => simplified,
        _ => healed,
    }
}
