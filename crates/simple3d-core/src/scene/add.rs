//! Putting new nodes into the tree.

use super::*;
use crate::mesh_data::MeshData;
use crate::primitive;
use std::sync::Arc;

impl Scene {
    pub fn add_primitive(&mut self, type_id: &str, parent: NodeId, index: usize) -> Option<NodeId> {
        let spec = primitive::lookup(type_id)?;
        let name = self.unique_name(spec.label);
        let body = Body::Primitive { type_id: type_id.to_string(), params: spec.default_params() };
        Some(self.insert_fresh(name, body, parent, index))
    }

    pub fn add_group(&mut self, op: GroupOp, parent: NodeId, index: usize) -> NodeId {
        let name = self.unique_name("Group");
        self.insert_fresh(name, Body::Group { op }, parent, index)
    }

    /// Add a pattern node (issue 67), starting as a default linear pattern.
    pub fn add_pattern(&mut self, parent: NodeId, index: usize) -> NodeId {
        let name = self.unique_name("Pattern");
        self.insert_fresh(name, Body::Pattern { params: crate::pattern::default_params() }, parent, index)
    }

    /// Add a node owning the given geometry (issue 80).
    pub fn add_mesh(&mut self, name: &str, mesh: MeshData, parent: NodeId, index: usize) -> NodeId {
        let name = self.unique_name(name);
        self.add_mesh_named(name, mesh, parent, index)
    }

    /// The same, with the name taken as given (issue 82). For split pieces: `unique_name` would cost
    /// a hundred million comparisons for ten thousand pieces and would advance the objects' numbering.
    pub fn add_mesh_named(&mut self, name: String, mesh: MeshData, parent: NodeId, index: usize) -> NodeId {
        self.insert_fresh(name, Body::Mesh { mesh: Arc::new(mesh) }, parent, index)
    }

    /// Add a default-placed node holding `body` under `parent` at `index`.
    pub(super) fn insert_fresh(&mut self, name: String, body: Body, parent: NodeId, index: usize) -> NodeId {
        let id = self.fresh_id();
        self.nodes.insert(id, Node::fresh(id, name, body, Some(parent)));
        self.link(id, parent, index);
        id
    }
}
