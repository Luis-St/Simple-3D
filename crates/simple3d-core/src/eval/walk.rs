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
    /// Walk the tree accumulating each node's world frame, its own mesh in world
    /// space and its local bounding box. One pass, so the transforms the handles
    /// use and the meshes picking uses can never disagree.
    pub(super) fn walk(&mut self, scene: &Scene, id: NodeId, parent: Xform, out: &mut Collected, cancel: &Cancel) {
        if cancel.is_cancelled() {
            return;
        }
        out.frames.insert(id, parent);
        let node = scene.node(id);
        // The anchor shift happens in the node's own frame, before its rotation,
        // so it composes on the right of the node's own transform.
        let anchor_offset = match node.anchor {
            Anchor::Base => self.subtree(scene, id, cancel).anchor_offset,
            Anchor::Centre => Vec3::ZERO,
        };
        let own = parent.compose(&Xform::from_pos_rot_scale(
            node.position,
            node.rotation,
            crate::scene::Node::sane_scale(node.scale),
        ));
        let shifted = own.compose(&Xform::from_translation(anchor_offset));
        match &node.body {
            Body::Primitive { .. } => {
                let mesh = self.primitive_mesh(scene, id);
                if let Some((lo, hi)) = mesh.bounds() {
                    out.local_bounds.insert(id, (lo + anchor_offset, hi + anchor_offset));
                }
                let mut world = apply(&shifted, &mesh);
                world.set_tag(crate::scene::colour_tag(scene.effective_colour(id)));
                let world = Arc::new(world);
                if let Some(bounds) = world.bounds() {
                    out.world_bounds.insert(id, bounds);
                }
                out.meshes.insert(id, world);
            }
            // A stored mesh: its own geometry in world space, so picking, the
            // selection outline and the ghost display all reach it.
            Body::Mesh { .. } => {
                let subtree = self.subtree(scene, id, cancel);
                let inv =
                    Xform::from_pos_rot_scale(node.position, node.rotation, crate::scene::Node::sane_scale(node.scale))
                        .inverse();
                if let Some((lo, hi)) = subtree.mesh.bounds() {
                    let (a, b) = (inv.point(lo), inv.point(hi));
                    out.local_bounds.insert(id, (a.min(b), a.max(b)));
                }
                let world = Arc::new(apply(&parent, &subtree.mesh));
                if let Some(bounds) = world.bounds() {
                    out.world_bounds.insert(id, bounds);
                }
                out.meshes.insert(id, world);
            }
            // A split is walked as the group it behaves like: its pieces are
            // its children, and the node itself outlines what they make.
            Body::Group { .. } | Body::Split { .. } => {
                let subtree = self.subtree(scene, id, cancel);
                // The group's own result, kept so the selection outline can draw
                // the shape the group *is*. It stays in the parent's frame:
                // `parent` is what `node_frames` records for this node, so the
                // pair is enough to place it, and sharing the `Arc` with the
                // subtree cache costs nothing.
                out.group_meshes.insert(id, subtree.mesh.clone());
                if let Some((lo, hi)) = subtree.mesh.bounds() {
                    // A group's mesh is already in its parent's frame, so undo
                    // the node's own transform to get its local box.
                    let inv = Xform::from_pos_rot_scale(
                        node.position,
                        node.rotation,
                        crate::scene::Node::sane_scale(node.scale),
                    )
                    .inverse();
                    let (a, b) = (inv.point(lo), inv.point(hi));
                    out.local_bounds.insert(id, (a.min(b), a.max(b)));
                    // World bounds are measured over the transformed *points*,
                    // not by transporting the box: rotating a box's corners and
                    // taking their extent would report a group under an angled
                    // ancestor as bigger than it is.
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
                // The whole repeated mesh in world space, so a click on any copy
                // -- not just the original the child sits at -- selects the
                // pattern. The child meshes are still collected below, so the
                // original copy also reaches the child that draws it.
                let world = Arc::new(apply(&parent, &subtree.mesh));
                out.meshes.insert(id, world);
                for &child in &node.children {
                    self.walk(scene, child, shifted, out, cancel);
                }
            }
        }
    }
}
