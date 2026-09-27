//! Evaluating one subtree: the boolean of its children, or its leaf primitive's mesh.

use super::*;
use crate::scene::{Anchor, Body, GroupOp, NodeId, Scene};
use simple3d_geom::{Mesh, Vec3};
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;

impl Evaluator {
    /// A node's mesh in its parent's frame: geometry, anchor, rotation, position. Anchoring before
    /// rotating makes anchor changes move only the origin.
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
        // Each step makes a new mesh, so the cached one is only read.
        let mut placed = std::borrow::Cow::Borrowed(&*local.mesh);

        let anchor_offset = match (node.anchor, placed.bounds()) {
            (Anchor::Base, Some((lo, _))) => Vec3::new(0.0, 0.0, -lo.z),
            _ => Vec3::ZERO,
        };
        if anchor_offset.z != 0.0 {
            placed = std::borrow::Cow::Owned(placed.translated(anchor_offset));
        }
        // Same order as `Xform::from_pos_rot_scale`, so the manipulator's frames agree with the mesh.
        // Scaling after anchoring keeps a base-anchored shape on z = 0.
        let scale = crate::scene::Node::sane_scale(node.scale);
        if scale != Vec3::ONE {
            placed = std::borrow::Cow::Owned(placed.scaled(scale));
        }
        let mesh = placed.transformed(node.position, node.rotation);
        // Placement keeps vertex order, so the children's ranges still hold.
        let result = Arc::new(SubtreeResult { mesh: Arc::new(mesh), anchor_offset, errors, passed });
        // Nothing computed under cancellation is cached: an abandoned boolean returns an empty mesh,
        // and caching it would make the shapes vanish until the content hash changed.
        if !cancel.is_cancelled() {
            self.subtrees.insert(key, result.clone());
        }
        result
    }
}

impl Evaluator {
    /// A node's own-frame result from its body and children, or the errors so far if abandoned.
    ///
    /// Groups, splits and patterns are cached by content (`Evaluator::locals`), so a moved node hits
    /// the cache. Primitives and stored meshes are recoloured copies, cheaper to redo than keep.
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
        // Children passed through untouched, by visible index, with where their vertices landed.
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
                // The generated mesh is shared between identical primitives, so colour goes on this copy only.
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
            // A split's pieces are appended, not unioned (issue 82). They are disjoint by construction,
            // and a union would weld neighbours back into the original shape, slowly, on every edit.
            // Appending also keeps each piece its own body, colour, tag and shell.
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
            // The component's result placed at the integration's origin (issue 113), evaluated as its own
            // tab does, with this node's placement on top.
            Body::Component { component, op } => match scene.linked_component(*component).cloned() {
                Some(inner) => {
                    let root = inner.root();
                    let own = inner.node(root).group_op();
                    let result = match op {
                        Some(op) if Some(*op) != own => {
                            let mut changed = (*inner).clone();
                            if let Some(root) = changed.get_mut(root) {
                                root.body = Body::Group { op: *op };
                            }
                            self.subtree(&changed, root, cancel)
                        }
                        _ => self.subtree(&inner, root, cancel),
                    };
                    // The component's nodes are not in this scene, so errors are reported on this node.
                    errors.extend(result.errors.iter().map(|e| NodeError {
                        node: id,
                        name: node.name.clone(),
                        message: format!("{}: {}", e.name, e.message),
                    }));
                    if cancel.is_cancelled() {
                        return Err(errors);
                    }
                    let mut mesh = (*result.mesh).clone();
                    if let Some(colour) = node.colour {
                        mesh.set_tag(colour.tag());
                    }
                    mesh
                }
                None => {
                    errors.push(NodeError {
                        node: id,
                        name: node.name.clone(),
                        message: "The component it places is no longer in the project".into(),
                    });
                    Mesh::new()
                }
            },
            Body::Pattern { params } => {
                // The repeated unit: children placed and appended, not booleaned, so this stays fast.
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
                    // Checked per copy, since the count is typed and may be too large to run to the end.
                    if cancel.is_cancelled() {
                        return Err(errors);
                    }
                    let mut copy = apply(&instance.xform, &unit);
                    // A reflection reverses the winding, so the faces are flipped back.
                    if instance.mirrored {
                        copy.flip_winding();
                    }
                    copies.push(copy);
                }
                // Copies are unioned, to match manual duplicates in a union group: touching copies must merge
                // into a clean solid, and concatenating made shared edges non-manifold. Non-overlapping copies
                // are free, since `union_all` rejects them on their boxes before the BSP kernel.
                combine(GroupOp::Union, &copies, id, &node.name, &mut errors, cancel).0
            }
        };
        let local = Arc::new(LocalResult { mesh: Arc::new(mesh), errors, passed, node: id });
        // Cached only when not abandoned, as in `subtree`.
        if let (Some(key), false) = (key, cancel.is_cancelled()) {
            self.locals.insert(key, local.clone());
        }
        Ok(local)
    }
}

impl Evaluator {
    /// Which vertices of `id`'s mesh each node below it owns ([`Evaluated::ranges`]), with `id`
    /// starting at `start`. Walked from the tree, since cached results are shared across nodes.
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
