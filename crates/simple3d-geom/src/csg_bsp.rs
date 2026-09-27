//! A self-contained BSP boolean kernel (union, subtract, intersect) after Evan Wallace's csg.js.
//! Written here since `csgrs` failed to build from crates.io and a C++ kernel would break the
//! nothing-to-install constraint.
//!
//! Known limitation: fixed-epsilon plane classification means near-degenerate inputs can still
//! yield non-manifold results; the evaluator detects them with `Mesh::manifold_issue` and fails on
//! that node rather than emit broken geometry.

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

const EPSILON: f64 = 1e-8;

/// Polygons clipped between cancellation checks (see [`crate::Abandon`]).
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
    // Heal the T-junctions whole-polygon clipping leaves, so every result (and chained input) is
    // edge-manifold. See `repair`.
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

/// The three operations, abandoned when `give_up` says so (see [`crate::Abandon`]).
pub fn union_until(a: &Mesh, b: &Mesh, give_up: crate::Abandon<'_>) -> Mesh {
    op(a, b, BoolOp::Union, give_up)
}

pub fn subtract_until(a: &Mesh, b: &Mesh, give_up: crate::Abandon<'_>) -> Mesh {
    op(a, b, BoolOp::Subtract, give_up)
}

pub fn intersect_until(a: &Mesh, b: &Mesh, give_up: crate::Abandon<'_>) -> Mesh {
    op(a, b, BoolOp::Intersect, give_up)
}
