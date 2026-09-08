//! A subtree on its own: exported to a document, read back from one, or
//! put in place of the whole scene.

use super::*;
use crate::mesh_data::{MeshBlob, MeshData};
use crate::primitive::{self, Params};
use simple3d_geom::tiling::SplitPlan;
use std::sync::Arc;

impl Scene {
    /// A group's base child in a difference: the first *visible* one (spec
    /// section 3.3). The property editor states this plainly.
    pub fn difference_base(&self, group: NodeId) -> Option<NodeId> {
        self.nodes.get(&group)?.children.iter().copied().find(|c| self.nodes[c].visible)
    }

    // -- portable form (project file and clipboard share one schema) ---------

    pub fn export_subtree(&self, id: NodeId) -> Option<NodeData> {
        let node = self.nodes.get(&id)?;
        let mut blob: Option<MeshBlob> = None;
        let mut original: Option<Box<NodeData>> = None;
        let mut tiling: Option<SplitPlan> = None;
        let (type_id, op, params) = match &node.body {
            Body::Group { op } => ("group".to_string(), Some(*op), Params::new()),
            Body::Primitive { type_id, params } => (type_id.clone(), None, params.clone()),
            Body::Pattern { params } => ("pattern".to_string(), None, params.clone()),
            Body::Mesh { mesh } => {
                blob = Some(mesh.to_blob());
                ("mesh".to_string(), None, Params::new())
            }
            Body::Split { original: was, plan } => {
                original = Some(Box::new((**was).clone()));
                tiling = plan.clone();
                ("split".to_string(), None, Params::new())
            }
        };
        Some(NodeData {
            name: node.name.clone(),
            type_id,
            op,
            position: node.position,
            rotation: node.rotation,
            scale: node.scale,
            anchor: node.anchor,
            visible: node.visible,
            ghost: node.ghost,
            colour: node.colour.map(Colour::to_hex),
            segments: node.segments,
            export_body: node.export_body,
            extracted: node.extracted,
            mesh: blob,
            original,
            tiling,
            params,
            children: node.children.iter().filter_map(|&c| self.export_subtree(c)).collect(),
        })
    }

    /// Insert a portable subtree, giving every node a fresh identity. Unknown
    /// primitive types are rejected so a corrupt or newer file cannot produce a
    /// half-loaded scene.
    pub fn import_subtree(&mut self, data: &NodeData, parent: NodeId, index: usize) -> Option<NodeId> {
        let body = match data.type_id.as_str() {
            "group" => Body::Group { op: data.op.unwrap_or_default() },
            "pattern" => Body::Pattern { params: crate::pattern::migrate_params(&data.params) },
            // A mesh whose blob cannot be read is refused rather than loaded as
            // an empty node: the geometry is the whole of what the node is, and
            // a silently empty one would be a body quietly missing from a print.
            "mesh" => Body::Mesh { mesh: Arc::new(MeshData::from_blob(data.mesh.as_ref()?)?) },
            // A split without the object it was made from is refused for the
            // same reason: what the node *is* is missing. Its pieces would still
            // draw, but it would be a shape broken apart with no way back, which
            // is not the node the file says it is.
            "split" => {
                Body::Split { original: Arc::new((**data.original.as_ref()?).clone()), plan: data.tiling.clone() }
            }
            type_id => {
                let spec = primitive::lookup(type_id)?;
                Body::Primitive { type_id: data.type_id.clone(), params: spec.migrate_params(&data.params) }
            }
        };
        let id = self.fresh_id();
        let node = Node {
            id,
            name: if data.name.is_empty() { "Node".to_string() } else { data.name.clone() },
            position: data.position,
            rotation: data.rotation,
            scale: Node::sane_scale(data.scale),
            anchor: data.anchor,
            visible: data.visible,
            ghost: data.ghost,
            colour: data.colour.as_deref().and_then(Colour::from_hex),
            segments: data.segments,
            export_body: data.export_body,
            extracted: data.extracted,
            body,
            children: Vec::new(),
            parent: Some(parent),
        };
        self.nodes.insert(id, node);
        self.link(id, parent, index);
        for (i, child) in data.children.iter().enumerate() {
            if self.import_subtree(child, id, i).is_none() {
                self.remove(id);
                return None;
            }
        }
        Some(id)
    }

    /// Replace the whole tree from a portable root, used when loading a project.
    pub fn replace_root(&mut self, data: &NodeData) -> Option<()> {
        let mut fresh = Scene::new();
        fresh.settings = self.settings.clone();
        fresh.camera = self.camera;
        {
            let root = fresh.nodes.get_mut(&fresh.root).unwrap();
            root.name = data.name.clone();
            root.body = Body::Group { op: data.op.unwrap_or_default() };
            root.position = data.position;
            root.rotation = data.rotation;
            root.scale = Node::sane_scale(data.scale);
            root.anchor = data.anchor;
            root.visible = data.visible;
            root.segments = data.segments;
        }
        let root = fresh.root;
        for (i, child) in data.children.iter().enumerate() {
            fresh.import_subtree(child, root, i)?;
        }
        self.nodes = fresh.nodes;
        self.root = fresh.root;
        self.next_id = fresh.next_id;
        Some(())
    }
}
