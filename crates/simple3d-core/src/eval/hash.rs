//! The key a subtree is cached under: everything that can change its mesh, and nothing else.

use super::*;
use crate::primitive::ParamValue;
use crate::scene::{Anchor, Body, NodeId, Scene};
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
    }

    fn hash_content(&self, scene: &Scene, id: NodeId, hasher: &mut Hasher64) {
        let node = scene.node(id);
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
