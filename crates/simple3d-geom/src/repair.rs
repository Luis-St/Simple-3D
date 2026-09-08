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

mod weld;
pub use weld::weld_tolerant;
pub(crate) use weld::*;
mod faces;
pub(crate) use faces::*;
mod junctions;
pub(crate) use junctions::*;
mod pinch;
pub(crate) use pinch::*;
mod cap;
pub(crate) use cap::*;
#[cfg(test)]
mod tests;

use crate::mesh::Mesh;
use crate::vec3::Vec3;

/// Positions closer than this are the same point. Boolean intersection points
/// are computed from `f64` plane arithmetic, so two evaluations of the same
/// physical point agree to ~1e-12mm; 1e-6mm is far below any dimension a user
/// can enter and far above that noise.
pub const WELD_TOL: f64 = 1e-6;

type Cell = (i64, i64, i64);

fn cell_of(p: Vec3, size: f64) -> Cell {
    ((p.x / size).floor() as i64, (p.y / size).floor() as i64, (p.z / size).floor() as i64)
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
    heal_until(mesh, &crate::never)
}

/// The same, giving up between attempts when the answer is no longer wanted.
/// Four attempts over a mesh of a hundred thousand triangles is where the rest
/// of a boolean's time goes once the clipping is done, so a cancellation that
/// only landed in the kernel would still leave the interface waiting on this.
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
