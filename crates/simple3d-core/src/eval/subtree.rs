//! Evaluating one subtree: the boolean of its children, or the mesh of the
//! primitive at its leaf.

use super::*;
use crate::scene::{Anchor, Body, GroupOp, NodeId, Scene};
use simple3d_geom::{Mesh, Vec3};
use std::sync::Arc;

impl Evaluator {
    /// The mesh of a node in its *parent's* frame: generated geometry, then the
    /// anchor, then rotation, then position. Anchoring before rotating is what
    /// makes changing the anchor move only the origin, never the shape.
    pub(super) fn subtree(&mut self, scene: &Scene, id: NodeId, cancel: &Cancel) -> Arc<SubtreeResult> {
        let key = self.subtree_key(scene, id);
        if let Some(hit) = self.subtrees.get(&key) {
            return hit.clone();
        }
        if cancel.is_cancelled() {
            return Arc::new(SubtreeResult {
                mesh: Arc::new(Mesh::new()),
                anchor_offset: Vec3::ZERO,
                errors: Vec::new(),
            });
        }

        let node = scene.node(id);
        let mut errors: Vec<NodeError> = Vec::new();
        let mut local = match &node.body {
            Body::Mesh { mesh } => {
                let mut copy = mesh.mesh.clone();
                if let Some(colour) = scene.effective_colour(id) {
                    copy.set_tag(colour.tag());
                }
                copy
            }
            Body::Primitive { .. } => {
                // The generated mesh is cached by its shape alone and shared
                // between identical primitives, so the colour is stamped on the
                // copy this node keeps, never on the cached original.
                let mut mesh = (*self.primitive_mesh(scene, id)).clone();
                mesh.set_tag(crate::scene::colour_tag(scene.effective_colour(id)));
                mesh
            }
            Body::Group { .. } => {
                let op = node.combine_op().unwrap_or_default();
                let mut child_meshes: Vec<Mesh> = Vec::new();
                for &child in &node.children {
                    if !scene.node(child).visible {
                        continue;
                    }
                    let child_result = self.subtree(scene, child, cancel);
                    errors.extend(child_result.errors.iter().cloned());
                    if child_result.mesh.triangle_count() > 0 {
                        child_meshes.push((*child_result.mesh).clone());
                    }
                }
                if cancel.is_cancelled() {
                    return Arc::new(SubtreeResult { mesh: Arc::new(Mesh::new()), anchor_offset: Vec3::ZERO, errors });
                }
                combine(op, &child_meshes, id, &node.name, &mut errors, cancel)
            }
            // A split is its pieces, side by side -- appended, not unioned
            // (issue 82).
            //
            // The pieces were cut out of one solid by cells that do not
            // overlap, so they are disjoint by construction and a union has
            // nothing to resolve. What it does instead is undo the split: every
            // pair of them meets along a whole face, the kernel welds the pair
            // into one body, and what comes back out is the shape they were cut
            // from -- a plate cut into 378 squares evaluated to the twelve
            // triangles of the plate, in a tenth of a second in release and
            // seconds in a debug build, on every edit of the scene.
            //
            // Appending is what a pattern does with its unit for the same
            // reason, and it keeps each piece a body of its own: its own
            // colour, its own tag, its own shell for the exporter to write.
            // Nothing is welded, so nothing shares an edge between two pieces
            // and the result stays manifold shell by shell.
            Body::Split { .. } => {
                let mut pieces = Mesh::new();
                for &child in &node.children {
                    if !scene.node(child).visible {
                        continue;
                    }
                    let child_result = self.subtree(scene, child, cancel);
                    errors.extend(child_result.errors.iter().cloned());
                    pieces.append(&child_result.mesh);
                    if cancel.is_cancelled() {
                        return Arc::new(SubtreeResult {
                            mesh: Arc::new(Mesh::new()),
                            anchor_offset: Vec3::ZERO,
                            errors,
                        });
                    }
                }
                pieces
            }
            Body::Pattern { params } => {
                // The unit the pattern repeats: its children, placed by their own
                // positions and appended. A pattern lays copies side by side, it
                // does not boolean them, so this is a concatenation and stays fast
                // however many copies there are.
                let mut unit = Mesh::new();
                for &child in &node.children {
                    if !scene.node(child).visible {
                        continue;
                    }
                    let child_result = self.subtree(scene, child, cancel);
                    errors.extend(child_result.errors.iter().cloned());
                    unit.append(&child_result.mesh);
                }
                if cancel.is_cancelled() {
                    return Arc::new(SubtreeResult { mesh: Arc::new(Mesh::new()), anchor_offset: Vec3::ZERO, errors });
                }
                let mut copies: Vec<Mesh> = Vec::new();
                for instance in crate::pattern::instances(params) {
                    // Checked per copy, not just before the loop: a pattern is
                    // the one node whose cost is a number someone types, so a
                    // count that turns out to be too large has to be abandonable
                    // rather than run to the end.
                    if cancel.is_cancelled() {
                        return Arc::new(SubtreeResult {
                            mesh: Arc::new(Mesh::new()),
                            anchor_offset: Vec3::ZERO,
                            errors,
                        });
                    }
                    let mut copy = apply(&instance.xform, &unit);
                    // A reflection reverses the winding, so its faces point the
                    // wrong way until they are flipped back.
                    if instance.mirrored {
                        copy.flip_winding();
                    }
                    copies.push(copy);
                }
                // The copies are *unioned*, not concatenated. A pattern stands in
                // for manual duplicates, so it has to produce what those would:
                // duplicates dropped in a union group meet the kernel as separate
                // operands and come out one clean solid, and copies that merely
                // touch -- which is what a step equal to the shape's own width
                // gives -- must do the same. Concatenating them instead welded
                // the contact into an edge shared by four triangles, and the
                // enclosing group then reported the whole scene non-manifold.
                //
                // This costs nothing for the copies that stand clear of each
                // other, which is the ordinary case and the one a helix makes
                // many of: `union_all` rejects non-overlapping operands on their
                // bounding boxes and never enters the BSP kernel for them.
                combine(GroupOp::Union, &copies, id, &node.name, &mut errors, cancel)
            }
        };

        let anchor_offset = match (node.anchor, local.bounds()) {
            (Anchor::Base, Some((lo, _))) => Vec3::new(0.0, 0.0, -lo.z),
            _ => Vec3::ZERO,
        };
        if anchor_offset.z != 0.0 {
            local = local.translated(anchor_offset);
        }
        // Anchor, then scale, then rotate, then translate -- the same order
        // `Xform::from_pos_rot_scale` composes, so the per-node world frames the
        // manipulator uses and the mesh agree. Scaling *after* the anchor is what
        // keeps a base-anchored shape standing on z = 0 whatever it is scaled by.
        let scale = crate::scene::Node::sane_scale(node.scale);
        if scale != Vec3::ONE {
            local = local.scaled(scale);
        }
        let mesh = local.transformed(node.position, node.rotation);
        let result = Arc::new(SubtreeResult { mesh: Arc::new(mesh), anchor_offset, errors });
        // Nothing computed under cancellation is kept, however finished it
        // looks. An abandoned boolean gives back an empty mesh, and the
        // evaluator -- and so this cache -- outlives the run that was
        // abandoned: cached, that empty mesh is what every later evaluation of
        // the same content gets back, so the shapes vanish from the viewport
        // and stay vanished until something changes the content hash. The
        // cancelled run's own answer is dropped by the worker; this is the
        // other half of dropping it.
        if !cancel.is_cancelled() {
            self.subtrees.insert(key, result.clone());
        }
        result
    }
}
