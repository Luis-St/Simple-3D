//! The key a subtree is cached under: everything that can change its mesh,
//! and nothing that cannot.

use super::*;
use crate::primitive::ParamValue;
use crate::scene::{Anchor, Body, NodeId, Scene};
use simple3d_geom::Vec3;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

impl Evaluator {
    /// Content hash of a subtree, including everything that affects geometry
    /// and nothing that does not -- a node's name is not in here, so renaming
    /// costs no re-evaluation.
    pub(super) fn subtree_key(&self, scene: &Scene, id: NodeId) -> u64 {
        let mut hasher = Hasher64::new();
        self.hash_subtree(scene, id, &mut hasher);
        hasher.finish()
    }

    pub(super) fn hash_subtree(&self, scene: &Scene, id: NodeId, hasher: &mut Hasher64) {
        let node = scene.node(id);
        hash_vec3(hasher, node.position);
        hash_vec3(hasher, node.rotation);
        hash_vec3(hasher, crate::scene::Node::sane_scale(node.scale));
        (node.anchor == Anchor::Base).hash(&mut hasher.0);
        match &node.body {
            Body::Primitive { type_id, params } => {
                type_id.hash(&mut hasher.0);
                hash_params(hasher, params);
                // The colour rides on the mesh as a per-triangle tag, so a
                // repaint has to miss the cache the way a resize does.
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
                // Length of the visible child list, so hiding the last child of
                // a union is not confused with having one fewer child.
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
            // Neither the recipe a split carries nor the tiling that made its
            // pieces is geometry -- nothing evaluates either until the pieces
            // are joined back together -- so both stay out of the key, and two
            // splits holding the same pieces share one result.
            Body::Split { .. } => {
                "split".hash(&mut hasher.0);
                for &child in &node.children {
                    if scene.node(child).visible {
                        self.hash_subtree(scene, child, hasher);
                    }
                }
                node.children.iter().filter(|c| scene.node(**c).visible).count().hash(&mut hasher.0);
            }
            Body::Mesh { mesh } => {
                "mesh".hash(&mut hasher.0);
                // The geometry itself never changes -- a stored mesh is
                // immutable, and an edit to one replaces it -- so its identity
                // is the allocation it lives in plus how big it is. Hashing a
                // hundred thousand vertices on every keystroke would cost more
                // than rebuilding the shapes the cache exists to avoid.
                (Arc::as_ptr(mesh) as usize).hash(&mut hasher.0);
                mesh.triangle_count().hash(&mut hasher.0);
                crate::scene::colour_tag(scene.effective_colour(id)).hash(&mut hasher.0);
            }
        }
    }
}

/// `DefaultHasher::new` uses fixed keys (unlike `RandomState`), so the same
/// input hashes the same in every process -- which is what lets the cache key be
/// compared across runs and keeps evaluation reproducible.
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
    // Normalise -0.0 to 0.0 and NaN to a single pattern so equal values always
    // hash equal.
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
