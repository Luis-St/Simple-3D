//! The key a subtree is cached under: everything that can change its mesh, and nothing else.

use super::*;
use crate::primitive::ParamValue;
use crate::scene::{Anchor, Body, NodeId, ObjectEdit, Scene};
use simple3d_geom::Vec3;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

impl Evaluator {
    /// Content hash of a subtree, covering exactly what affects geometry; names are excluded.
    pub(super) fn subtree_key(&self, scene: &Scene, id: NodeId) -> u64 {
        let mut hasher = Hasher64::new();
        self.hash_subtree(scene, id, &mut hasher);
        hasher.finish()
    }

    /// Content hash of what a node makes before placement, so moving a group reuses its boolean.
    pub(super) fn content_key(&self, scene: &Scene, id: NodeId) -> u64 {
        let mut hasher = Hasher64::new();
        self.hash_content(scene, id, &mut hasher);
        hasher.finish()
    }

    pub(super) fn hash_subtree(&self, scene: &Scene, id: NodeId, hasher: &mut Hasher64) {
        let node = scene.node(id);
        hash_vec3(hasher, node.position);
        hash_vec3(hasher, node.rotation);
        hash_vec3(hasher, crate::scene::Node::sane_scale(node.scale));
        (node.anchor == Anchor::Base).hash(&mut hasher.0);
        self.hash_content(scene, id, hasher);
        // Applied after the body's result, so they are in this key and not in the content key.
        node.edits.len().hash(&mut hasher.0);
        for edit in &node.edits {
            match edit {
                ObjectEdit::Push(edit) => {
                    edit.placing.hash(&mut hasher.0);
                    hash_f64(hasher, edit.distance);
                    hash_vec3(hasher, edit.position);
                    hash_vec3(hasher, edit.rotation);
                    hash_outline(hasher, &edit.outline);
                }
                ObjectEdit::Round(edit) => hash_round(hasher, edit),
            }
        }
    }

    fn hash_content(&self, scene: &Scene, id: NodeId, hasher: &mut Hasher64) {
        let node = scene.node(id);
        // The node itself, for the bodies whose triangles are stamped with it (issue 73): a cached copy
        // made for a look-alike elsewhere would name the wrong object. Costs sharing between identical
        // shapes, which the per-primitive mesh cache still does.
        if source_stamped(scene, id) {
            id.hash(&mut hasher.0);
        }
        match &node.body {
            Body::Primitive { type_id, params } => {
                type_id.hash(&mut hasher.0);
                hash_params(hasher, params);
                // Colour rides on the mesh as a tag, so a repaint must miss the cache.
                crate::scene::colour_tag(scene.effective_colour(id)).hash(&mut hasher.0);
                let spec = crate::primitive::lookup(type_id);
                if spec.map_or(false, |s| s.segmented) {
                    scene.segments_for(id).hash(&mut hasher.0);
                }
            }
            Body::Group { op } => {
                "group".hash(&mut hasher.0);
                (*op as u8).hash(&mut hasher.0);
                for &child in &node.children {
                    if scene.node(child).visible {
                        self.hash_subtree(scene, child, hasher);
                    }
                }
                // The visible child count, so hiding the last child is distinguishable.
                node.children.iter().filter(|c| scene.node(**c).visible).count().hash(&mut hasher.0);
            }
            Body::Pattern { params } => {
                "pattern".hash(&mut hasher.0);
                hash_params(hasher, params);
                for &child in &node.children {
                    if scene.node(child).visible {
                        self.hash_subtree(scene, child, hasher);
                    }
                }
                node.children.iter().filter(|c| scene.node(**c).visible).count().hash(&mut hasher.0);
            }
            // A split's recipe and tiling are not geometry until joined, so they stay out of the key.
            Body::Split { .. } => {
                "split".hash(&mut hasher.0);
                for &child in &node.children {
                    if scene.node(child).visible {
                        self.hash_subtree(scene, child, hasher);
                    }
                }
                node.children.iter().filter(|c| scene.node(**c).visible).count().hash(&mut hasher.0);
            }
            // An integration's key includes the component's whole tree, so edits in its tab miss the cache.
            Body::Component { component, op } => {
                "component".hash(&mut hasher.0);
                op.map(|op| op as u8).hash(&mut hasher.0);
                node.colour.map(|c| c.tag()).hash(&mut hasher.0);
                match scene.linked_component(*component) {
                    Some(inner) => self.hash_subtree(inner, inner.root(), hasher),
                    None => component.hash(&mut hasher.0),
                }
            }
            Body::Extrusion { outline, params } => {
                "extrusion".hash(&mut hasher.0);
                hash_params(hasher, params);
                hash_outline(hasher, outline);
                crate::scene::colour_tag(scene.effective_colour(id)).hash(&mut hasher.0);
            }
            Body::Mesh { mesh } => {
                "mesh".hash(&mut hasher.0);
                // Stored meshes are immutable, so their allocation and size identify them; hashing every vertex
                // per keystroke would cost too much.
                (Arc::as_ptr(mesh) as usize).hash(&mut hasher.0);
                mesh.triangle_count().hash(&mut hasher.0);
                crate::scene::colour_tag(scene.effective_colour(id)).hash(&mut hasher.0);
            }
        }
    }
}

/// Whether a node's result carries its own id as every triangle's source rather than its children's:
/// leaves, and the bodies whose result is one new surface or is picked as one (hull, pattern,
/// component).
pub(crate) fn source_stamped(scene: &Scene, id: NodeId) -> bool {
    match &scene.node(id).body {
        Body::Group { op } => *op == crate::scene::GroupOp::Hull,
        Body::Split { .. } => false,
        _ => true,
    }
}

fn hash_round(hasher: &mut Hasher64, edit: &crate::scene::RoundEdit) {
    edit.kind.hash(&mut hasher.0);
    hash_f64(hasher, edit.size);
    edit.segments.hash(&mut hasher.0);
    edit.edges.len().hash(&mut hasher.0);
    for edge in &edit.edges {
        for v in [edge.a, edge.b, edge.normals[0], edge.normals[1], edge.along[0], edge.along[1]] {
            hash_vec3(hasher, v);
        }
        edge.convex.hash(&mut hasher.0);
    }
    edit.joints.len().hash(&mut hasher.0);
    edit.joints.iter().for_each(|&p| hash_vec3(hasher, p));
    edit.corners.len().hash(&mut hasher.0);
    for corner in &edit.corners {
        hash_vec3(hasher, corner.at);
        for &(dir, length) in &corner.edges {
            hash_vec3(hasher, dir);
            hash_f64(hasher, length);
        }
        corner.faces.iter().for_each(|&n| hash_vec3(hasher, n));
    }
}

fn hash_outline(hasher: &mut Hasher64, outline: &simple3d_geom::push_pull::Outline) {
    for point in outline.outer.iter().chain(outline.holes.iter().flatten()) {
        hash_f64(hasher, point[0]);
        hash_f64(hasher, point[1]);
    }
    outline.outer.len().hash(&mut hasher.0);
    outline.holes.iter().map(Vec::len).collect::<Vec<_>>().hash(&mut hasher.0);
}

/// The source number a node's triangles carry (issue 73); node ids stay far below `u32::MAX`.
pub fn source_of(id: NodeId) -> u32 {
    id as u32
}

/// `DefaultHasher::new` uses fixed keys, so hashes match across processes and evaluation is reproducible.
pub(crate) struct Hasher64(pub(super) std::collections::hash_map::DefaultHasher);

impl Hasher64 {
    pub(super) fn new() -> Hasher64 {
        Hasher64(std::collections::hash_map::DefaultHasher::new())
    }

    pub(super) fn finish(&self) -> u64 {
        self.0.finish()
    }
}

pub(crate) fn hash_f64(hasher: &mut Hasher64, v: f64) {
    // Normalise -0.0 and NaN so equal values hash equal.
    let v = if v == 0.0 { 0.0 } else { v };
    if v.is_nan() { u64::MAX } else { v.to_bits() }.hash(&mut hasher.0);
}

pub(crate) fn hash_vec3(hasher: &mut Hasher64, v: Vec3) {
    hash_f64(hasher, v.x);
    hash_f64(hasher, v.y);
    hash_f64(hasher, v.z);
}

pub(crate) fn hash_params(hasher: &mut Hasher64, params: &crate::primitive::Params) {
    for (key, value) in params {
        key.hash(&mut hasher.0);
        match value {
            ParamValue::Length(v) | ParamValue::Angle(v) => hash_f64(hasher, *v),
            ParamValue::Count(v) | ParamValue::Choice(v) => v.hash(&mut hasher.0),
            ParamValue::Bool(b) => b.hash(&mut hasher.0),
        }
    }
}
