//! Scene evaluation (spec section 5.2).
//!
//! Turns the node tree into one mesh. Three properties matter and are all
//! tested here:
//!
//! * **Deterministic.** The same tree always produces the same mesh, so the
//!   cache key can be a hash of the subtree and two runs are comparable.
//! * **Cached per subtree**, invalidated only where the tree actually changed,
//!   so editing one dimension does not re-evaluate the whole scene.
//! * **Cancellable.** Evaluation runs off the interaction path and is
//!   superseded cleanly when the user edits again while one is running.
//!
//! A boolean that cannot be evaluated fails loudly on its own node -- named, so
//! the outliner can show it -- while every other branch still previews. It never
//! emits geometry it knows to be broken.

mod evaluated;
pub use evaluated::Evaluated;
mod cancel;
pub use cancel::Cancel;
mod hash;
mod run;
mod subtree;
mod walk;
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

/// A node-specific evaluation failure, carrying the name so the interface can
/// say which node is at fault rather than showing a generic message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeError {
    pub node: NodeId,
    pub name: String,
    pub message: String,
}

/// Holds the caches between runs. Keys are content hashes, so a node that moved
/// in the tree but did not change still hits.
pub struct Evaluator {
    primitives: BTreeMap<u64, Arc<Mesh>>,
    subtrees: BTreeMap<u64, Arc<SubtreeResult>>,
    /// Bounded so a long editing session cannot grow without limit. Entries are
    /// pure functions of their key, so dropping any of them is always safe.
    pub cache_limit: usize,
}

impl Default for Evaluator {
    fn default() -> Self {
        Evaluator::new()
    }
}

#[derive(Debug)]
struct SubtreeResult {
    /// The subtree's mesh in its *parent's* frame.
    mesh: Arc<Mesh>,
    /// The shift the `Base` anchor applied in the node's own frame, before
    /// rotation. Kept so the per-node world transforms agree with the mesh.
    anchor_offset: Vec3,
    errors: Vec<NodeError>,
}
