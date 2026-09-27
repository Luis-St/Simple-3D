//! Retriangulating the flat regions of a boolean result.
//!
//! BSP clipping by infinite planes slices whole faces, so a plate with a hole and a boss comes out
//! at ~1500 triangles instead of ~300, and every later boolean pays for them. Each flat region is
//! rebuilt from its boundary, which is kept exactly (including T-junction vertices, so healing is
//! not undone); only strictly interior vertices are dropped. Regions may be concave with holes, so
//! holes are bridged and the result ear-clipped; anything uninterpretable is left as it was.

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

/// What makes two triangles one flat region: their plane and their body.
type FlatRegion = ((i64, i64, i64, i64), u32);

/// Rebuild every flat region of `mesh` from its boundary. Uninterpretable regions keep their
/// triangles, so this never fails.
pub fn retriangulate_flat_regions(mesh: &Mesh) -> Mesh {
    // Keyed by plane and tag, so flush bodies of different colours are not merged.
    let mut groups: BTreeMap<FlatRegion, Vec<usize>> = BTreeMap::new();
    let mut ungrouped: Vec<([u32; 3], u32)> = Vec::new();
    for (i, t) in mesh.indices.iter().enumerate() {
        match plane_of(mesh, *t) {
            Some((normal, w)) => groups.entry((plane_key(normal, w), mesh.tag(i))).or_default().push(i),
            // Degenerate: no usable plane. Left as it was.
            None => ungrouped.push((*t, mesh.tag(i))),
        }
    }

    let mut out = ungrouped;
    for (&(_, tag), tris) in groups.iter() {
        let original = || tris.iter().map(|&i| (mesh.indices[i], mesh.tag(i)));
        if tris.len() < 3 {
            // One or two triangles: nothing to gain.
            out.extend(original());
            continue;
        }
        let normal = plane_of(mesh, mesh.indices[tris[0]]).unwrap().0;
        match boundary_loops(mesh, tris).and_then(|loops| triangulate_region(&mesh.positions, normal, loops)) {
            Some(rebuilt) if rebuilt.len() <= tris.len() => out.extend(rebuilt.into_iter().map(|t| (t, tag))),
            // Uninterpretable, or rebuilt with more triangles: keep the original.
            _ => out.extend(original()),
        }
    }
    let (indices, tags) = out.into_iter().unzip();
    Mesh { positions: mesh.positions.clone(), indices, tags }
}
