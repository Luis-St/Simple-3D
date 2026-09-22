//! Running an evaluation, and the cache it keeps between runs.

use super::*;
use crate::scene::Scene;
use crate::xform::Xform;
use std::collections::BTreeMap;

impl Evaluator {
    pub fn new() -> Evaluator {
        Evaluator { primitives: BTreeMap::new(), subtrees: BTreeMap::new(), worlds: BTreeMap::new(), cache_limit: 4096 }
    }

    pub fn cached_subtrees(&self) -> usize {
        self.subtrees.len()
    }

    pub fn clear(&mut self) {
        self.primitives.clear();
        self.subtrees.clear();
        self.worlds.clear();
    }

    /// `source` placed with `frame`, and painted with `tag` when there is one:
    /// the mesh from the last run if all three are what it was made from.
    pub(super) fn world_mesh(&mut self, id: NodeId, source: &Arc<Mesh>, frame: Xform, tag: Option<u32>) -> Arc<Mesh> {
        if let Some(last) = self.worlds.get(&id) {
            if Arc::ptr_eq(&last.source, source) && last.frame == frame && last.tag == tag {
                return last.world.clone();
            }
        }
        let mut world = apply(&frame, source);
        if let Some(tag) = tag {
            world.set_tag(tag);
        }
        let world = Arc::new(world);
        self.worlds.insert(id, WorldMesh { source: source.clone(), frame, tag, world: world.clone() });
        world
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
        // A node no longer in the tree has nothing to be reused for.
        self.worlds.retain(|id, _| collected.meshes.contains_key(id));
        self.trim();
        Evaluated {
            mesh: result.mesh.clone(),
            bounds: result.mesh.bounds(),
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
