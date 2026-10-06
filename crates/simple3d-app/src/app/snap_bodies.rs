//! The bodies snapping and measuring see: the model as drawn, not the shapes it is made from.
//!
//! A boolean, a hull or a pattern is one body with its evaluated result, since its operands have
//! corners and edges the result does not: a cutter's, and those of what it cut away (issue 70's
//! path picking found this; snapping and measuring had the same fault). Assemblies and splits are
//! their parts, which stay separate bodies.

use super::*;
use simple3d_core::scene::{Body, GroupOp, NodeId};
use simple3d_geom::Mesh;
use std::sync::Arc;

impl App {
    /// Every shown body with its world mesh. A body holding one of `exclude` (the nodes being dragged)
    /// is taken apart into its parts instead, so the dragged shape is left out and the rest still
    /// catches; its result cannot be told apart from the dragged part's.
    pub(crate) fn snap_bodies(&self, exclude: &[NodeId]) -> Vec<(NodeId, Arc<Mesh>)> {
        let mut out = Vec::new();
        let root = self.scene.root();
        for &child in &self.scene.node(root).children {
            self.collect_bodies(child, exclude, &mut out);
        }
        out
    }

    /// The bodies `id` is made of, for the features a dragged node carries with it.
    pub(crate) fn bodies_of(&self, id: NodeId) -> Vec<(NodeId, Arc<Mesh>)> {
        let mut out = Vec::new();
        self.collect_bodies(id, &[], &mut out);
        out
    }

    fn collect_bodies(&self, id: NodeId, exclude: &[NodeId], out: &mut Vec<(NodeId, Arc<Mesh>)>) {
        let Some(node) = self.scene.get(id) else { return };
        if !node.visible || exclude.contains(&id) {
            return;
        }
        let whole = match &node.body {
            Body::Group { op } => *op != GroupOp::Assembly,
            Body::Split { .. } => false,
            Body::Primitive { .. }
            | Body::Mesh { .. }
            | Body::Component { .. }
            | Body::Pattern { .. }
            | Body::Extrusion { .. } => true,
        };
        let holds_excluded = exclude.iter().any(|&dragged| self.scene.is_ancestor_of(id, dragged));
        if whole && !holds_excluded {
            if let Some(mesh) = self.body_mesh(id) {
                out.push((id, mesh));
                return;
            }
        }
        for &child in &node.children {
            self.collect_bodies(child, exclude, out);
        }
    }

    /// A body's world mesh: a shape's own, or a group's result carried into the world, made once per
    /// result and frame.
    pub(crate) fn body_mesh(&self, id: NodeId) -> Option<Arc<Mesh>> {
        if let Some(mesh) = self.evaluated.node_meshes.get(&id) {
            return Some(mesh.clone());
        }
        let local = self.evaluated.group_meshes.get(&id)?;
        let frame = self.evaluated.node_frames.get(&id)?;
        let key = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            (Arc::as_ptr(local) as usize).hash(&mut hasher);
            frame.hash_bits(&mut hasher);
            hasher.finish()
        };
        if let Some((held, mesh)) = self.body_meshes.borrow().get(&id) {
            if *held == key {
                return Some(mesh.clone());
            }
        }
        let mesh = Arc::new(Mesh {
            positions: local.positions.iter().map(|&p| frame.point(p)).collect(),
            indices: local.indices.clone(),
            tags: local.tags.clone(),
            sources: local.sources.clone(),
        });
        self.body_meshes.borrow_mut().insert(id, (key, mesh.clone()));
        Some(mesh)
    }
}
