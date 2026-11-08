//! The brush's working copy of a mesh body (issue 127): what a stroke edits many times a second
//! instead of the document.
//!
//! A stroke moves vertices of a mutable copy the session owns; the document only receives the
//! result when the stroke ends, as one undo step, so nothing provisional is ever in an undo snapshot
//! or a save. While a stroke runs the copy is drawn directly, standing in for the node's committed
//! geometry (`render::Live::hidden`), and only what changed goes to the card (`render::Edits`).
//!
//! Moving vertices keeps the triangles, so the connectivity ([`Topology`]), the vertex normals and
//! the picking tree are built once per session and then updated around the moved vertices only.
//! They live here rather than on [`MeshData`]: only the brush reads them (the renderer builds its own
//! edge table from the placed mesh), and every stroke ends in a *new* `MeshData`, so a cache on it
//! would be thrown away right after being built. The session instead outlives its strokes: while
//! the node still holds the mesh the last stroke wrote, the next stroke starts from the copy and
//! caches as they are.

mod app;
mod build;
mod detail;
mod region;
#[cfg(test)]
mod tests;
mod update;

use crate::pick::{Bvh, Refit};
use crate::render::{Edits, Renderable};
use simple3d_core::mesh_data::MeshData;
use simple3d_core::scene::NodeId;
use simple3d_core::xform::Xform;
use simple3d_geom::topology::Topology;
use simple3d_geom::Vec3;
use std::sync::Arc;

/// A mesh body's working copy, in world space, with everything a dab reads kept current.
pub(crate) struct Sculpt {
    pub target: NodeId,
    /// The mesh the copy was made from or last wrote back. While the node holds it (and is placed
    /// and coloured as before), the copy is still that mesh and the session carries on.
    pub base: Arc<MeshData>,
    /// From the mesh's own coordinates to the world, as the evaluation places it (`App::mesh_placement`).
    pub placement: Xform,
    /// The node's colour tag the copy is drawn with, if it has one.
    pub colour: Option<u32>,
    /// Each triangle's tag as the node stores it, written back with the result.
    tags: Vec<u32>,
    /// The copy as drawn: its mesh is the working copy, welded, triangles numbered as in `base`.
    pub renderable: Renderable,
    pub topology: Topology,
    /// Each vertex's unit normal ([`simple3d_geom::topology::vertex_normals`]), for moving along it.
    pub normals: Vec<Vec3>,
    /// Which triangles shade smooth: all of them, as for every mesh body (`Evaluated::smooth`).
    smooth: Vec<bool>,
    /// The picking tree over the copy, refit after every dab rather than built again.
    tree: Bvh,
    refit: Refit,
    /// Visit marks for [`Sculpt::within`], and the mark of the current flood.
    visited: Vec<u32>,
    visit: u32,
    /// What changed since the card last had the copy.
    pub edits: Edits,
    /// The stroke in progress.
    pub stroke: Option<Stroke>,
    /// The node's committed geometry is not one stretch of the scene (it sits in a boolean or a
    /// pattern), so it cannot be hidden: the copy is drawn over it and the boolean shows the stroke
    /// only once it ends.
    pub lagging: bool,
    /// The stroke was written back and the copy is still drawn until the evaluation shows it.
    pub settling: bool,
}

/// One stroke: where each vertex it moved was before, and facing which way, so Cancel can put it
/// back and the noise mode can measure from there.
#[derive(Default)]
pub(crate) struct Stroke {
    /// Each vertex's place in `before`, or `u32::MAX` while the stroke has not moved it.
    seen: Vec<u32>,
    before: Vec<(u32, Vec3, Vec3)>,
    /// Detail was added: the triangles changed, so Cancel makes the copy again from `base`.
    reshaped: bool,
    /// How strongly each vertex may move, from the brush's keep settings (`shaping::weights`).
    pub weights: Vec<f64>,
}

impl Stroke {
    /// Where vertex `v` was when the stroke first moved it, and its normal then; `None` if not yet.
    pub fn rest(&self, v: u32) -> Option<(Vec3, Vec3)> {
        let at = *self.seen.get(v as usize)?;
        self.before.get(at as usize).map(|&(_, position, normal)| (position, normal))
    }

    /// Whether the stroke has changed anything yet.
    pub fn is_empty(&self) -> bool {
        self.before.is_empty() && !self.reshaped
    }
}

impl Sculpt {
    /// Whether the copy is drawn in place of the committed geometry.
    pub fn shown(&self) -> bool {
        self.stroke.is_some() || self.settling
    }

    /// The nearest triangle the ray hits on the copy as it is now, and how far along.
    pub fn ray(&self, origin: Vec3, dir: Vec3) -> Option<(f64, usize)> {
        self.tree.nearest_triangle(&self.renderable.mesh, origin, dir)
    }
}
