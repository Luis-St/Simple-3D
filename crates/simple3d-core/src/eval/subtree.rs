//! Evaluating one subtree: the boolean of its children, or the mesh of the
//! primitive at its leaf.

use super::*;
use crate::scene::{Anchor, Body, GroupOp, NodeId, Scene};
use simple3d_geom::{Mesh, Vec3};
use std::collections::BTreeMap;
use std::ops::Range;
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
            return Arc::new(SubtreeResult::abandoned(Vec::new()));
        }

        let node = scene.node(id);
        let local = match self.local(scene, id, cancel) {
            Ok(local) => local,
            Err(errors) => return Arc::new(SubtreeResult::abandoned(errors)),
        };
        let (errors, passed) = (local.errors.clone(), local.passed.clone());
        // Each step below makes a new mesh, so the cached one is only read.
        let mut placed = std::borrow::Cow::Borrowed(&*local.mesh);

        let anchor_offset = match (node.anchor, placed.bounds()) {
            (Anchor::Base, Some((lo, _))) => Vec3::new(0.0, 0.0, -lo.z),
            _ => Vec3::ZERO,
        };
        if anchor_offset.z != 0.0 {
            placed = std::borrow::Cow::Owned(placed.translated(anchor_offset));
        }
        // Anchor, then scale, then rotate, then translate -- the same order
        // `Xform::from_pos_rot_scale` composes, so the per-node world frames the
        // manipulator uses and the mesh agree. Scaling *after* the anchor is what
        // keeps a base-anchored shape standing on z = 0 whatever it is scaled by.
        let scale = crate::scene::Node::sane_scale(node.scale);
        if scale != Vec3::ONE {
            placed = std::borrow::Cow::Owned(placed.scaled(scale));
        }
        let mesh = placed.transformed(node.position, node.rotation);
        // Placing a mesh moves its vertices and keeps their order, so where
        // each child's landed still holds.
        let result = Arc::new(SubtreeResult { mesh: Arc::new(mesh), anchor_offset, errors, passed });
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

impl Evaluator {
    /// What a node makes of its body and its children, in its own frame and
    /// before it is anchored or placed -- or, when the run is abandoned part
    /// way, the errors found so far.
    ///
    /// A group's, a split's and a pattern's are cached by their content alone
    /// (`Evaluator::locals`), so a node that has only moved gets its boolean
    /// back from the cache. A primitive's and a stored mesh's are a copy with
    /// the colour stamped on, which costs less to make again than to keep.
    fn local(&mut self, scene: &Scene, id: NodeId, cancel: &Cancel) -> Result<Arc<LocalResult>, Vec<NodeError>> {
        let cached = matches!(scene.node(id).body, Body::Group { .. } | Body::Split { .. } | Body::Pattern { .. });
        let key = cached.then(|| self.content_key(scene, id));
        if let Some(hit) = key.and_then(|key| self.locals.get(&key)) {
            if hit.errors.is_empty() || hit.node == id {
                return Ok(hit.clone());
            }
        }
        let node = scene.node(id);
        let mut errors: Vec<NodeError> = Vec::new();
        // The children whose geometry comes through into this mesh untouched,
        // by their place among the visible children, with where their
        // vertices landed in it.
        let mut passed: Vec<(usize, u32)> = Vec::new();
        let mesh = match &node.body {
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
                let mut places: Vec<usize> = Vec::new();
                let visible = node.children.iter().filter(|&&child| scene.node(child).visible);
                for (place, &child) in visible.enumerate() {
                    let child_result = self.subtree(scene, child, cancel);
                    errors.extend(child_result.errors.iter().cloned());
                    if child_result.mesh.triangle_count() > 0 {
                        child_meshes.push((*child_result.mesh).clone());
                        places.push(place);
                    }
                }
                if cancel.is_cancelled() {
                    return Err(errors);
                }
                let (mesh, untouched) = combine(op, &child_meshes, id, &node.name, &mut errors, cancel);
                passed.extend(untouched.into_iter().map(|(index, offset)| (places[index], offset)));
                mesh
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
                let visible = node.children.iter().filter(|&&child| scene.node(child).visible);
                for (place, &child) in visible.enumerate() {
                    let child_result = self.subtree(scene, child, cancel);
                    errors.extend(child_result.errors.iter().cloned());
                    passed.push((place, pieces.positions.len() as u32));
                    pieces.append(&child_result.mesh);
                    if cancel.is_cancelled() {
                        return Err(errors);
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
                    return Err(errors);
                }
                let mut copies: Vec<Mesh> = Vec::new();
                for instance in crate::pattern::instances(params) {
                    // Checked per copy, not just before the loop: a pattern is
                    // the one node whose cost is a number someone types, so a
                    // count that turns out to be too large has to be abandonable
                    // rather than run to the end.
                    if cancel.is_cancelled() {
                        return Err(errors);
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
                combine(GroupOp::Union, &copies, id, &node.name, &mut errors, cancel).0
            }
        };
        let local = Arc::new(LocalResult { mesh: Arc::new(mesh), errors, passed, node: id });
        // Kept only when the run was not abandoned, for the reason `subtree`
        // gives for its own cache.
        if let (Some(key), false) = (key, cancel.is_cancelled()) {
            self.locals.insert(key, local.clone());
        }
        Ok(local)
    }
}

impl Evaluator {
    /// Which vertices of `id`'s mesh each node below it is -- see
    /// [`Evaluated::ranges`] -- with `id` itself starting at `start`.
    ///
    /// Found by walking the tree rather than kept with the cached results:
    /// those are shared by every node with the same content, so a result
    /// knows which of its children came through, but not which nodes they
    /// are.
    pub(super) fn ranges(
        &mut self,
        scene: &Scene,
        id: NodeId,
        start: u32,
        cancel: &Cancel,
        out: &mut BTreeMap<NodeId, Range<u32>>,
    ) {
        let result = self.subtree(scene, id, cancel);
        out.insert(id, start..start + result.mesh.positions.len() as u32);
        let visible: Vec<NodeId> =
            scene.node(id).children.iter().copied().filter(|&child| scene.node(child).visible).collect();
        for &(place, offset) in &result.passed {
            if let Some(&child) = visible.get(place) {
                self.ranges(scene, child, start + offset, cancel, out);
            }
        }
    }
}
