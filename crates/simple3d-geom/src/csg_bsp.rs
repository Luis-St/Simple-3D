//! Self-contained BSP-tree boolean CSG kernel (union / subtract / intersect),
//! following the classic algorithm popularised by Evan Wallace's csg.js and
//! used by many browser-based CAD tools. We don't depend on an external CSG
//! crate: at the time this was written every published version of the one
//! obvious crate (`csgrs`) pulls in a yanked transitive dependency and fails
//! to build from crates.io, and pulling in a big C++ kernel would break the
//! "fully self-contained, nothing to install" constraint. This kernel is
//! deliberately small and easy to audit instead.
//!
//! Known limitation: plane classification uses a fixed epsilon rather than
//! exact/rational arithmetic, so pathologically thin or near-degenerate
//! inputs can in principle still produce a non-manifold result. `Mesh::manifold_issue`
//! is used by the evaluator to detect that and fail loudly on the offending
//! node rather than emit broken geometry, per the spec's requirement.

mod plane;
pub(crate) use plane::*;
mod polygon;
pub(crate) use polygon::*;
mod splitter;
pub(crate) use splitter::*;
mod box_tree;
pub(crate) use box_tree::*;
mod box_tree_query;
pub(crate) use box_tree_query::*;
mod box_tree_gather;
mod convex;
pub(crate) use convex::*;
mod node;
pub(crate) use node::*;
mod clip_convex;
mod node_build;
mod node_clip;
mod node_query;
pub(crate) use clip_convex::*;
mod clip_near;
pub(crate) use clip_near::*;
mod merge;
pub(crate) use merge::*;
mod convert;
pub(crate) use convert::*;
pub use convert::{debug_mesh_to_polygons, debug_roundtrip, debug_splits_nothing};

use crate::mesh::Mesh;

const EPSILON: f64 = 1e-5;

/// How many polygons a clip gets through between asking whether the answer is
/// still wanted. See [`crate::Abandon`].
const ABANDON_EVERY: u32 = 256;

fn op(a: &Mesh, b: &Mesh, kind: BoolOp, give_up: crate::Abandon<'_>) -> Mesh {
    let mut na = BspNode::new(mesh_to_polygons(a));
    let mut nb = BspNode::new(mesh_to_polygons(b));
    let scratch = &mut ClipScratch::default();
    let polys = match kind {
        BoolOp::Union => {
            na.clip_to(&nb, scratch, give_up);
            nb.clip_to(&na, scratch, give_up);
            nb.invert();
            nb.clip_to(&na, scratch, give_up);
            nb.invert();
            let mut polys = na.all_polygons();
            polys.extend(nb.all_polygons());
            polys
        }
        BoolOp::Subtract => {
            na.invert();
            na.clip_to(&nb, scratch, give_up);
            nb.clip_to(&na, scratch, give_up);
            nb.invert();
            nb.clip_to(&na, scratch, give_up);
            nb.invert();
            na.invert();
            let mut polys = na.all_polygons();
            polys.extend(nb.all_polygons().iter().map(Polygon::flip));
            polys
        }
        BoolOp::Intersect => {
            na.invert();
            nb.clip_to(&na, scratch, give_up);
            nb.invert();
            na.clip_to(&nb, scratch, give_up);
            nb.clip_to(&na, scratch, give_up);
            na.invert();
            let mut polys = na.all_polygons();
            polys.extend(nb.all_polygons().iter().map(Polygon::flip));
            polys
        }
    };
    if give_up() {
        return Mesh::new();
    }
    // The BSP clips whole polygons, which leaves T-junctions wherever two
    // polygons sharing an edge were split at different points along it; heal
    // them here so every boolean result -- including one feeding the next
    // boolean in a chain -- is edge-manifold. See `repair`.
    crate::repair::heal_until(&polygons_to_mesh(&polys), give_up)
}

enum BoolOp {
    Union,
    Subtract,
    Intersect,
}

pub fn union(a: &Mesh, b: &Mesh) -> Mesh {
    op(a, b, BoolOp::Union, &crate::never)
}

pub fn subtract(a: &Mesh, b: &Mesh) -> Mesh {
    op(a, b, BoolOp::Subtract, &crate::never)
}

pub fn intersect(a: &Mesh, b: &Mesh) -> Mesh {
    op(a, b, BoolOp::Intersect, &crate::never)
}

/// The three of them again, abandoned part-way when `give_up` says so. See
/// [`crate::Abandon`].
pub fn union_until(a: &Mesh, b: &Mesh, give_up: crate::Abandon<'_>) -> Mesh {
    op(a, b, BoolOp::Union, give_up)
}

pub fn subtract_until(a: &Mesh, b: &Mesh, give_up: crate::Abandon<'_>) -> Mesh {
    op(a, b, BoolOp::Subtract, give_up)
}

pub fn intersect_until(a: &Mesh, b: &Mesh, give_up: crate::Abandon<'_>) -> Mesh {
    op(a, b, BoolOp::Intersect, give_up)
}
