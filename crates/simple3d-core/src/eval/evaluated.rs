//! What an evaluation produced: the meshes, the bounds, and the errors.

use super::*;
use crate::scene::NodeId;
use crate::xform::Xform;
use simple3d_geom::{Mesh, Vec3};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Evaluated {
    /// The whole scene as one mesh, ready to export.
    pub mesh: Arc<Mesh>,
    /// `mesh`'s bounding box, measured once, since the interface asks for it every frame.
    pub bounds: Option<(Vec3, Vec3)>,
    /// Each primitive's mesh in world space, for picking, highlighting and ghosts (hidden nodes
    /// included).
    pub node_meshes: BTreeMap<NodeId, Arc<Mesh>>,
    /// Each node's parent frame in world space, including ancestor anchor shifts; manipulators need it
    /// and its inverse.
    pub node_frames: BTreeMap<NodeId, Xform>,
    /// What each group evaluates to, in its `node_frames` frame: its result, not its operands, for the
    /// selection outline. Shared with the subtree cache, so recording it is an `Arc` clone.
    pub group_meshes: BTreeMap<NodeId, Arc<Mesh>>,
    /// Each node's bounding box in its own frame, after anchoring and before rotation and position:
    /// where the resize handles sit.
    pub node_local_bounds: BTreeMap<NodeId, (Vec3, Vec3)>,
    /// Each node's world-space bounding box, groups included: the measured size shown in the property
    /// editor.
    pub node_world_bounds: BTreeMap<NodeId, (Vec3, Vec3)>,
    /// The vertex range of `mesh` for each node whose geometry passed through untouched to the root.
    /// Nodes combined in a boolean or pattern are absent. Shared positions with other nodes are
    /// checked for by the viewport.
    pub ranges: BTreeMap<NodeId, std::ops::Range<u32>>,
    /// Each node's placement in its parent's frame for this evaluation; with `node_frames`, tells how
    /// far a node has moved since.
    pub placements: BTreeMap<NodeId, Xform>,
    /// Nodes whose own evaluation failed. Non-empty means export must refuse.
    pub errors: Vec<NodeError>,
    /// Set when the run was superseded by a later edit; the result is partial and should be discarded.
    pub cancelled: bool,
}

impl Evaluated {
    pub fn error_for(&self, node: NodeId) -> Option<&NodeError> {
        self.errors.iter().find(|e| e.node == node)
    }

    /// A node's own result in world space, booleans and all, for the selection outline. Outlining a
    /// group's children instead would show cutters and uncut boxes the result does not contain.
    pub fn result_mesh(&self, id: NodeId) -> Option<std::borrow::Cow<'_, Mesh>> {
        if let Some(mesh) = self.node_meshes.get(&id) {
            return Some(std::borrow::Cow::Borrowed(mesh));
        }
        let mesh = self.group_meshes.get(&id)?;
        let frame = self.node_frames.get(&id)?;
        Some(std::borrow::Cow::Owned(Mesh {
            positions: mesh.positions.iter().map(|&p| frame.point(p)).collect(),
            indices: mesh.indices.clone(),
            tags: mesh.tags.clone(),
        }))
    }
}
