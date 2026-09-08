//! Putting new nodes into the tree.

use super::*;
use crate::mesh_data::MeshData;
use crate::primitive;
use simple3d_geom::Vec3;
use std::sync::Arc;

impl Scene {
    pub fn add_primitive(&mut self, type_id: &str, parent: NodeId, index: usize) -> Option<NodeId> {
        let spec = primitive::lookup(type_id)?;
        let id = self.fresh_id();
        let node = Node {
            id,
            name: self.unique_name(spec.label),
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
            anchor: Anchor::Centre,
            visible: true,
            ghost: false,
            colour: None,
            segments: None,
            export_body: None,
            extracted: false,
            body: Body::Primitive { type_id: type_id.to_string(), params: spec.default_params() },
            children: Vec::new(),
            parent: Some(parent),
        };
        self.nodes.insert(id, node);
        self.link(id, parent, index);
        Some(id)
    }

    pub fn add_group(&mut self, op: GroupOp, parent: NodeId, index: usize) -> NodeId {
        let id = self.fresh_id();
        let node = Node {
            id,
            name: self.unique_name("Group"),
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
            anchor: Anchor::Centre,
            visible: true,
            ghost: false,
            colour: None,
            segments: None,
            export_body: None,
            extracted: false,
            body: Body::Group { op },
            children: Vec::new(),
            parent: Some(parent),
        };
        self.nodes.insert(id, node);
        self.link(id, parent, index);
        id
    }

    /// Add a pattern node (issue 67), which repeats whatever children are put
    /// under it. It starts as a linear pattern with the default numbers.
    pub fn add_pattern(&mut self, parent: NodeId, index: usize) -> NodeId {
        let id = self.fresh_id();
        let node = Node {
            id,
            name: self.unique_name("Pattern"),
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
            anchor: Anchor::Centre,
            visible: true,
            ghost: false,
            colour: None,
            segments: None,
            export_body: None,
            extracted: false,
            body: Body::Pattern { params: crate::pattern::default_params() },
            children: Vec::new(),
            parent: Some(parent),
        };
        self.nodes.insert(id, node);
        self.link(id, parent, index);
        id
    }

    /// Add a node that owns the geometry it is given (issue 80).
    pub fn add_mesh(&mut self, name: &str, mesh: MeshData, parent: NodeId, index: usize) -> NodeId {
        let name = self.unique_name(name);
        self.add_mesh_named(name, mesh, parent, index)
    }

    /// The same, taking the name exactly as given (issue 82).
    ///
    /// For the pieces of a split, whose names are already unique among
    /// themselves -- "Box Piece 1" and up, numbered by where they sit in the
    /// collection. Running them through [`Scene::unique_name`] would do two
    /// things wrong: every piece would be compared against every name in the
    /// document, which for ten thousand of them is a hundred million string
    /// comparisons before the split even lands, and the numbering would be
    /// answered from the same series the objects use -- eighty pieces called
    /// "Box 1" to "Box 80" leave the next box a user adds to be called "Box 81".
    pub fn add_mesh_named(&mut self, name: String, mesh: MeshData, parent: NodeId, index: usize) -> NodeId {
        let id = self.fresh_id();
        let node = Node {
            id,
            name,
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
            anchor: Anchor::Centre,
            visible: true,
            ghost: false,
            colour: None,
            segments: None,
            export_body: None,
            extracted: false,
            body: Body::Mesh { mesh: Arc::new(mesh) },
            children: Vec::new(),
            parent: Some(parent),
        };
        self.nodes.insert(id, node);
        self.link(id, parent, index);
        id
    }
}
