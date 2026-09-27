//! Walking the tree into subtrees, and the mesh a primitive node makes.

use super::*;
use crate::scene::{Anchor, Body, NodeId, Scene};
use crate::xform::Xform;
use simple3d_geom::{Mesh, Vec3};
use std::hash::Hash;
use std::sync::Arc;

impl Evaluator {
    pub(super) fn primitive_mesh(&mut self, scene: &Scene, id: NodeId) -> Arc<Mesh> {
        let node = scene.node(id);
        let Body::Primitive { type_id, params } = &node.body else { return Arc::new(Mesh::new()) };
        let Some(spec) = crate::primitive::lookup(type_id) else { return Arc::new(Mesh::new()) };
        let segments = if spec.segmented { scene.segments_for(id) } else { 0 };
        let mut hasher = Hasher64::new();
        type_id.hash(&mut hasher.0);
        hash_params(&mut hasher, params);
        segments.hash(&mut hasher.0);
        let key = hasher.finish();
        if let Some(hit) = self.primitives.get(&key) {
            return hit.clone();
        }
        let mesh = Arc::new((spec.build)(params, segments));
        self.primitives.insert(key, mesh.clone());
        mesh
    }
}

impl Evaluator {
    /// Walk the tree collecting each node's world frame, world mesh and local bounds in one pass, so
    /// handles and picking can never disagree.
    pub(super) fn walk(&mut self, scene: &Scene, id: NodeId, parent: Xform, out: &mut Collected, cancel: &Cancel) {
        if cancel.is_cancelled() {
            return;
        }
        out.frames.insert(id, parent);
        let node = scene.node(id);
        // The anchor shift is in the node's own frame before rotation, so it composes on the right.
        let anchor_offset = match node.anchor {
            Anchor::Base => self.subtree(scene, id, cancel).anchor_offset,
            Anchor::Centre => Vec3::ZERO,
        };
        let placement =
            Xform::from_pos_rot_scale(node.position, node.rotation, crate::scene::Node::sane_scale(node.scale));
        out.placements.insert(id, placement);
        let own = parent.compose(&placement);
        let shifted = own.compose(&Xform::from_translation(anchor_offset));
        match &node.body {
            Body::Primitive { .. } => {
                let mesh = self.primitive_mesh(scene, id);
                if let Some((lo, hi)) = mesh.bounds() {
                    out.local_bounds.insert(id, (lo + anchor_offset, hi + anchor_offset));
                }
                let tag = crate::scene::colour_tag(scene.effective_colour(id));
                let world = self.world_mesh(id, &mesh, shifted, Some(tag));
                if let Some(bounds) = world.bounds() {
                    out.world_bounds.insert(id, bounds);
                }
                out.meshes.insert(id, world);
            }
            // A stored mesh or an integration (issue 113): its own world geometry, picked, outlined and moved as one.
            Body::Mesh { .. } | Body::Component { .. } => {
                let subtree = self.subtree(scene, id, cancel);
                let inv =
                    Xform::from_pos_rot_scale(node.position, node.rotation, crate::scene::Node::sane_scale(node.scale))
                        .inverse();
                if let Some((lo, hi)) = subtree.mesh.bounds() {
                    let (a, b) = (inv.point(lo), inv.point(hi));
                    out.local_bounds.insert(id, (a.min(b), a.max(b)));
                }
                let world = self.world_mesh(id, &subtree.mesh, parent, None);
                if let Some(bounds) = world.bounds() {
                    out.world_bounds.insert(id, bounds);
                }
                out.meshes.insert(id, world);
            }
            // A split is walked like a group: its pieces are its children.
            Body::Group { .. } | Body::Split { .. } => {
                let subtree = self.subtree(scene, id, cancel);
                // The group's result, kept for the selection outline, in the parent's frame (`node_frames`) and
                // sharing the cache's `Arc`.
                out.group_meshes.insert(id, subtree.mesh.clone());
                if let Some((lo, hi)) = subtree.mesh.bounds() {
                    // The group's mesh is in its parent's frame, so undo the node's transform for its local box.
                    let inv = Xform::from_pos_rot_scale(
                        node.position,
                        node.rotation,
                        crate::scene::Node::sane_scale(node.scale),
                    )
                    .inverse();
                    let (a, b) = (inv.point(lo), inv.point(hi));
                    out.local_bounds.insert(id, (a.min(b), a.max(b)));
                    // World bounds from the transformed points, not a transported box, which would overstate an
                    // angled group's size.
                    if let Some(bounds) = bounds_of(subtree.mesh.positions.iter().map(|&p| parent.point(p))) {
                        out.world_bounds.insert(id, bounds);
                    }
                }
                for &child in &node.children {
                    self.walk(scene, child, shifted, out, cancel);
                }
            }
            Body::Pattern { .. } => {
                // Bounds like a group, over the whole repeated result.
                let subtree = self.subtree(scene, id, cancel);
                if let Some((lo, hi)) = subtree.mesh.bounds() {
                    let inv = Xform::from_pos_rot_scale(
                        node.position,
                        node.rotation,
                        crate::scene::Node::sane_scale(node.scale),
                    )
                    .inverse();
                    let (a, b) = (inv.point(lo), inv.point(hi));
                    out.local_bounds.insert(id, (a.min(b), a.max(b)));
                    if let Some(bounds) = bounds_of(subtree.mesh.positions.iter().map(|&p| parent.point(p))) {
                        out.world_bounds.insert(id, bounds);
                    }
                }
                // The whole repeated mesh is pickable, so clicking any copy selects the pattern.
                let world = self.world_mesh(id, &subtree.mesh, parent, None);
                out.meshes.insert(id, world);
                for &child in &node.children {
                    self.walk(scene, child, shifted, out, cancel);
                }
            }
        }
    }
}
