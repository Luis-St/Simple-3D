//! The per-node renderables, kept for as long as the mesh they were made from.

use super::Renderable;
use simple3d_core::eval::Evaluated;
use simple3d_core::scene::NodeId;
use simple3d_core::xform::Xform;
use simple3d_geom::Mesh;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Which renderable of a node is wanted: its plain one (a ghost) or the one
/// with the edge adjacency a selection outline is drawn from.
pub type Wanted = (NodeId, bool);

/// Renderables of single nodes, shared between the interface and the
/// evaluation thread.
///
/// Preparing a renderable is a weld and several passes over the mesh, and the
/// viewport needs one for every selected, ticked, previewed or ghosted node.
/// They used to be rebuilt, every one of them, on the interface thread whenever
/// the evaluation or the selection changed -- so every frame of a drag that
/// brought back an evaluation also stalled on preparing the selection again,
/// even though the only mesh that had changed was the one being dragged.
///
/// Now each entry remembers the mesh it was made from, and is used again for as
/// long as the evaluation hands out that very mesh: the subtree cache gives an
/// unchanged node the same `Arc` from one run to the next, so an unchanged node
/// is never prepared twice. And the evaluation thread fills in the ones the
/// interface has said it wants before sending its result, so the interface
/// usually finds everything already made and prepares nothing at all.
#[derive(Clone, Default)]
pub struct RenderableCache {
    entries: Arc<Mutex<HashMap<Wanted, Entry>>>,
}

struct Entry {
    /// What the renderable was made from. Held, not merely compared by
    /// address: while the entry lives the mesh cannot be freed and its address
    /// handed to a different one.
    mesh: Arc<Mesh>,
    /// The frame a group's mesh was placed with, which moves without the mesh
    /// itself changing. `None` for a node with a world-space mesh of its own.
    frame: Option<Xform>,
    renderable: Arc<Renderable>,
}

/// Where a node's result comes from in an evaluation -- the same two places
/// [`Evaluated::result_mesh`] looks.
fn source(evaluated: &Evaluated, id: NodeId) -> Option<(Arc<Mesh>, Option<Xform>)> {
    if let Some(mesh) = evaluated.node_meshes.get(&id) {
        return Some((mesh.clone(), None));
    }
    let mesh = evaluated.group_meshes.get(&id)?;
    let frame = evaluated.node_frames.get(&id)?;
    Some((mesh.clone(), Some(*frame)))
}

impl RenderableCache {
    /// The renderable for `wanted` in `evaluated`, made now if it is not
    /// already. `None` when the node has no result in that evaluation.
    pub fn get(&self, evaluated: &Evaluated, wanted: Wanted) -> Option<Arc<Renderable>> {
        let (mesh, frame) = source(evaluated, wanted.0)?;
        if let Some(entry) = self.entries.lock().expect("the cache lock").get(&wanted) {
            if Arc::ptr_eq(&entry.mesh, &mesh) && entry.frame == frame {
                return Some(entry.renderable.clone());
            }
        }
        // Made outside the lock: the other thread may be preparing something
        // else meanwhile, and a large mesh takes a while.
        let result = evaluated.result_mesh(wanted.0)?;
        let renderable = Arc::new(match wanted.1 {
            true => Renderable::prepare_outlined(&result),
            false => Renderable::prepare(&result),
        });
        let entry = Entry { mesh, frame, renderable: renderable.clone() };
        self.entries.lock().expect("the cache lock").insert(wanted, entry);
        Some(renderable)
    }

    /// Forget every entry but the ones in `keep`, so a node that is no longer
    /// drawn does not hold its mesh and its renderable in memory.
    pub fn retain(&self, keep: &[Wanted]) {
        self.entries.lock().expect("the cache lock").retain(|wanted, _| keep.contains(wanted));
    }
}
