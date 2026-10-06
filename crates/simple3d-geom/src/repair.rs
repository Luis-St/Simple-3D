//! Post-boolean mesh repair.
//!
//! BSP booleans clip polygons against the other solid's tree independently, so two polygons
//! sharing an edge can be split at different points, leaving T-junctions: no gap, but not
//! edge-manifold, which slicers reject. `heal` fixes this afterwards by welding with a real
//! tolerance and splitting each triangle at vertices on its edges, in a single pass (see
//! `split_t_junctions`).

mod weld;
pub use weld::weld_tolerant;
pub(crate) use weld::*;
mod faces;
pub(crate) use faces::*;
mod junctions;
pub(crate) use junctions::*;
mod points;
pub(crate) use points::*;
mod pinch;
pub(crate) use pinch::*;
mod cap;
pub(crate) use cap::*;
#[cfg(test)]
mod tests;

use crate::mesh::Mesh;
use crate::vec3::Vec3;

/// Positions closer than this are the same point: far below any user dimension and far above the
/// ~1e-12 mm disagreement of `f64` plane arithmetic.
pub const WELD_TOL: f64 = 1e-6;

type Cell = (i64, i64, i64);

fn cell_of(p: Vec3, size: f64) -> Cell {
    ((p.x / size).floor() as i64, (p.y / size).floor() as i64, (p.z / size).floor() as i64)
}

/// Weld, cancel coincident opposite faces, remove T-junctions, and retriangulate flat regions,
/// so nested booleans get clean input.
///
/// If the result is still broken, retry from the original at coarser tolerances: chained booleans
/// drift copies of a point up to 3e-6 mm apart, which `WELD_TOL` misses, and which side of it they
/// land on differed between platforms. The ordinary case stops at the first attempt.
pub fn heal(mesh: &Mesh) -> Mesh {
    heal_until(mesh, &crate::never)
}

/// The same, giving up between attempts when the answer is no longer wanted, since healing a
/// large mesh is a large share of a boolean's time.
pub fn heal_until(mesh: &Mesh, give_up: crate::Abandon<'_>) -> Mesh {
    let best = heal_at(mesh, WELD_TOL);
    if best.manifold_issue().is_none() {
        return best;
    }
    for factor in [10.0, 100.0, 1000.0] {
        if give_up() {
            return best;
        }
        let again = heal_at(mesh, WELD_TOL * factor);
        if again.manifold_issue().is_none() {
            return again;
        }
    }
    best
}

/// One healing attempt at one tolerance.
fn heal_at(mesh: &Mesh, tol: f64) -> Mesh {
    let m = weld_tolerant(mesh, tol);
    let m = collapse_short_edges(m, tol * 4.0);
    let m = drop_slivers(m, tol);
    let m = cancel_opposite_faces(m);
    let healed = split_t_junctions(m, tol);

    // Retriangulating flat regions drops the collinear vertices inserted above (an ear clipper stalls
    // on them); a second T-junction pass puts the same splits back into fewer triangles. Compacted
    // first, since orphaned interior vertices would otherwise be split straight back in. Both
    // candidates are capped before choosing, since triangle counts do not say which caps cleanly.
    let simplified = split_t_junctions(compact(crate::planar::retriangulate_flat_regions(&healed)), tol);
    let simplified = split_needles(cap_boundary_loops(compact(simplified)), tol);
    if simplified.manifold_issue().is_none() {
        return simplified;
    }
    sounder_of(split_needles(cap_boundary_loops(compact(healed)), tol), simplified)
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
    Mesh { positions, indices, tags: mesh.tags.clone(), sources: mesh.sources.clone() }
}
