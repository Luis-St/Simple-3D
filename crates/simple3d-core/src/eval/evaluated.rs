//! What an evaluation produced: the meshes, the bounds, and what went wrong
//! where.

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
    /// `mesh`'s bounding box, measured once. The interface asks for the size of
    /// the scene on every frame -- the status bar, the document panel, the
    /// bounding box overlay and the section plane all show it -- and measuring
    /// it meant a pass over every vertex of the model each time.
    pub bounds: Option<(Vec3, Vec3)>,
    /// Each primitive's own mesh in world space, for picking, selection
    /// highlighting and the translucent display of hidden nodes. Hidden nodes are
    /// included so they can be drawn as ghosts.
    pub node_meshes: BTreeMap<NodeId, Arc<Mesh>>,
    /// Each node's *parent* frame in world space, including any anchor shift an
    /// ancestor group applied. A manipulator handle needs this to place itself,
    /// and its inverse to turn a world-space drag back into the parent-frame
    /// coordinates `Node::position` is stored in.
    pub node_frames: BTreeMap<NodeId, Xform>,
    /// What each *group* evaluates to, in the frame `node_frames` records for
    /// it -- its own boolean result, not its operands.
    ///
    /// A group has no surface of its own to click on, which is why it is not in
    /// `node_meshes`, but it does have a shape, and the selection outline has to
    /// draw that shape rather than the shapes that went into it. Held
    /// untransformed and shared with the subtree cache, so recording it costs an
    /// `Arc` rather than a copy of the geometry.
    pub group_meshes: BTreeMap<NodeId, Arc<Mesh>>,
    /// Each node's own bounding box in its own frame, after its anchor and before
    /// its rotation and position. This is what the resize handles sit on.
    pub node_local_bounds: BTreeMap<NodeId, (Vec3, Vec3)>,
    /// Each node's bounding box in world space -- what the property editor
    /// reports as the node's measured size. Present for groups as well as
    /// primitives: a group has no mesh of its own in `node_meshes`, but the
    /// assembly it evaluates to is exactly what a user asking "how big is this"
    /// means.
    pub node_world_bounds: BTreeMap<NodeId, (Vec3, Vec3)>,
    /// Which vertices of `mesh` each node's geometry is, for every node whose
    /// geometry came through into it untouched -- laid beside the rest, or
    /// unioned with nothing it touched, all the way up to the root. A node
    /// that went into a boolean with something else, or into a pattern, is
    /// not here: its geometry is not in `mesh` as it was.
    ///
    /// Its triangles are the ones that use those vertices, and they are all
    /// its own unless another node's geometry shares a position with it, which
    /// the viewport checks for before relying on it.
    pub ranges: BTreeMap<NodeId, std::ops::Range<u32>>,
    /// Each node's own placement in its parent's frame -- position, rotation
    /// and scale -- as it stood for this evaluation. With `node_frames`, what
    /// says how far a node has been moved since.
    pub placements: BTreeMap<NodeId, Xform>,
    /// Nodes whose own evaluation failed. Non-empty means export must refuse.
    pub errors: Vec<NodeError>,
    /// Set when the run was superseded by a later edit; the result is partial
    /// and should be discarded.
    pub cancelled: bool,
}

impl Evaluated {
    pub fn error_for(&self, node: NodeId) -> Option<&NodeError> {
        self.errors.iter().find(|e| e.node == node)
    }

    /// A node's own result in world space: what the node *is*, booleans and all.
    ///
    /// This is what the selection outline draws. Outlining a group by outlining
    /// its children instead draws shapes the result does not contain -- a
    /// difference's cutter as two rims hanging in mid-air where nothing is, an
    /// intersection's whole uncut box as a cage around the small lens it
    /// actually leaves.
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
