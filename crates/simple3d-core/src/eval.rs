//! Scene evaluation (spec section 5.2): the node tree to one mesh.
//!
//! * **Deterministic**, so cache keys can be content hashes and runs are comparable.
//! * **Cached per subtree**, invalidated only where the tree changed.
//! * **Cancellable**, superseded cleanly by a new edit.
//!
//! A failing boolean is reported on its own node while every other branch still previews; broken
//! geometry is never emitted.

mod evaluated;
pub use evaluated::Evaluated;
mod cancel;
pub use cancel::Cancel;
mod hash;
mod run;
mod edits;
mod subtree;
mod walk;
pub use hash::source_of;
pub(crate) use hash::*;
mod parts;
pub use parts::{body_meshes, part_meshes, Part};
mod bake;
pub(crate) use bake::*;
pub use bake::{baked_mesh, baked_mesh_in_place, selection_mesh, subtree_bounds};
#[cfg(test)]
mod tests;

use crate::scene::NodeId;
use simple3d_geom::{Mesh, Vec3};
use std::collections::BTreeMap;
use std::sync::Arc;

/// A node-specific evaluation failure, naming the node for the interface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeError {
    pub node: NodeId,
    pub name: String,
    pub message: String,
}

/// The caches between runs, keyed by content hash, so a node moved in the tree still hits.
pub struct Evaluator {
    primitives: BTreeMap<u64, Arc<Mesh>>,
    subtrees: BTreeMap<u64, Arc<SubtreeResult>>,
    /// What a group, split or pattern makes of its children before placement, by
    /// [`Evaluator::content_key`], so moving a group only redoes the placement, not its boolean.
    locals: BTreeMap<u64, Arc<LocalResult>>,
    /// Each node's world mesh from the last run with its sources. Unchanged nodes get the same `Arc`
    /// back, so the viewport and snapping recognise them by address.
    worlds: BTreeMap<NodeId, WorldMesh>,
    /// Cache bound for long sessions; entries are pure functions of their key, so dropping is safe.
    pub cache_limit: usize,
}

impl Default for Evaluator {
    fn default() -> Self {
        Evaluator::new()
    }
}

struct WorldMesh {
    /// Held rather than compared by address alone, so the address cannot be reused while remembered.
    source: Arc<Mesh>,
    frame: crate::xform::Xform,
    tag: Option<u32>,
    world: Arc<Mesh>,
}

#[derive(Debug)]
struct SubtreeResult {
    /// The subtree's mesh in its parent's frame.
    mesh: Arc<Mesh>,
    /// The `Base` anchor's shift in the node's frame before rotation, so world transforms agree.
    anchor_offset: Vec3,
    errors: Vec<NodeError>,
    /// The visible children passed through untouched, by index, with their first vertex. See
    /// [`Evaluated::ranges`].
    passed: Vec<(usize, u32)>,
}

/// A node's geometry in its own frame, before anchor, scale, rotation and position
/// (`Evaluator::locals`).
#[derive(Debug)]
struct LocalResult {
    mesh: Arc<Mesh>,
    errors: Vec<NodeError>,
    passed: Vec<(usize, u32)>,
    /// The node it was computed for; a result with errors is only reused for that node, since the
    /// errors name it.
    node: NodeId,
}

impl SubtreeResult {
    /// An abandoned run's result: nothing, and never cached.
    fn abandoned(errors: Vec<NodeError>) -> SubtreeResult {
        SubtreeResult { mesh: Arc::new(Mesh::new()), anchor_offset: Vec3::ZERO, errors, passed: Vec::new() }
    }
}
