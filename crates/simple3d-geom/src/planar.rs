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

mod regions;
pub(crate) use regions::*;
mod basis;
pub(crate) use basis::*;
mod triangulate;
pub use triangulate::triangulate_loops;
pub(crate) use triangulate::*;
mod holes;
pub(crate) use holes::*;
mod ear_clip;
pub(crate) use ear_clip::*;

use crate::mesh::Mesh;
use std::collections::BTreeMap;

/// Rebuild the triangulation of every flat region of `mesh` from its boundary.
/// Regions that cannot be interpreted keep their original triangles, so this
/// never fails -- at worst it changes nothing.
/// What makes two triangles part of the same flat region: the plane they lie
/// in and the body they came from.
type FlatRegion = ((i64, i64, i64, i64), u32);

pub fn retriangulate_flat_regions(mesh: &Mesh) -> Mesh {
    // Keyed by plane *and* tag: two bodies meeting flush in one plane are one
    // flat region geometrically but two differently painted ones, and merging
    // them would rebuild the boundary across a colour change.
    let mut groups: BTreeMap<FlatRegion, Vec<usize>> = BTreeMap::new();
    let mut ungrouped: Vec<([u32; 3], u32)> = Vec::new();
    for (i, t) in mesh.indices.iter().enumerate() {
        match plane_of(mesh, *t) {
            Some((normal, w)) => groups.entry((plane_key(normal, w), mesh.tag(i))).or_default().push(i),
            // Degenerate: no usable plane. Left exactly as it was.
            None => ungrouped.push((*t, mesh.tag(i))),
        }
    }

    let mut out = ungrouped;
    for (&(_, tag), tris) in groups.iter() {
        let original = || tris.iter().map(|&i| (mesh.indices[i], mesh.tag(i)));
        if tris.len() < 3 {
            // A one- or two-triangle region has nothing to gain and no interior
            // vertex to drop.
            out.extend(original());
            continue;
        }
        let normal = plane_of(mesh, mesh.indices[tris[0]]).unwrap().0;
        match boundary_loops(mesh, tris).and_then(|loops| triangulate_region(&mesh.positions, normal, loops)) {
            Some(rebuilt) if rebuilt.len() <= tris.len() => out.extend(rebuilt.into_iter().map(|t| (t, tag))),
            // Either the region was uninterpretable, or rebuilding it produced
            // *more* triangles than it started with -- in which case there was
            // nothing to win and the original is the safer answer.
            _ => out.extend(original()),
        }
    }
    let (indices, tags) = out.into_iter().unzip();
    Mesh { positions: mesh.positions.clone(), indices, tags }
}
