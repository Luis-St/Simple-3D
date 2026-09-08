//! Running an evaluation, and the cache it keeps between runs.

use super::*;
use crate::scene::Scene;
use crate::xform::Xform;
use std::collections::BTreeMap;

impl Evaluator {
    pub fn new() -> Evaluator {
        Evaluator { primitives: BTreeMap::new(), subtrees: BTreeMap::new(), cache_limit: 4096 }
    }

    pub fn cached_subtrees(&self) -> usize {
        self.subtrees.len()
    }

    pub fn clear(&mut self) {
        self.primitives.clear();
        self.subtrees.clear();
    }

    pub(super) fn trim(&mut self) {
        // Nothing here tracks recency: the caches exist to make an edit-to-
        // preview cycle fast, and after a wholesale clear the next run repopulates
        // exactly what the current tree needs.
        if self.subtrees.len() > self.cache_limit {
            self.subtrees.clear();
        }
        if self.primitives.len() > self.cache_limit {
            self.primitives.clear();
        }
    }

    pub fn evaluate(&mut self, scene: &Scene, cancel: &Cancel) -> Evaluated {
        let result = self.subtree(scene, scene.root(), cancel);
        let mut collected = Collected::default();
        self.walk(scene, scene.root(), Xform::IDENTITY, &mut collected, cancel);
        self.trim();
        Evaluated {
            mesh: result.mesh.clone(),
            node_meshes: collected.meshes,
            group_meshes: collected.group_meshes,
            node_frames: collected.frames,
            node_local_bounds: collected.local_bounds,
            node_world_bounds: collected.world_bounds,
            errors: result.errors.clone(),
            cancelled: cancel.is_cancelled(),
        }
    }
}
