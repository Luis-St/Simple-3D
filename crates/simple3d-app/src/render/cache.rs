//! The per-node renderables, kept as long as the mesh they came from.

use super::Renderable;
use simple3d_core::eval::Evaluated;
use simple3d_core::scene::NodeId;
use simple3d_core::xform::Xform;
use simple3d_geom::Mesh;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Which renderable of a node: plain (a ghost) or with outline adjacency (a selection).
pub type Wanted = (NodeId, bool);

/// Single-node renderables shared between the interface and the evaluation thread.
///
/// Each entry remembers its source mesh and is reused while the evaluation hands out that same
/// `Arc`, so unchanged nodes are never prepared twice. The evaluation thread also prepares the
/// requested ones before sending its result, so the interface rarely prepares anything.
#[derive(Clone, Default)]
pub struct RenderableCache {
    entries: Arc<Mutex<HashMap<Wanted, Entry>>>,
}

struct Entry {
    /// The source mesh, held so its address cannot be reused while the entry lives.
    mesh: Arc<Mesh>,
    /// The frame a group's mesh was placed with, which can move without the mesh changing.
    frame: Option<Xform>,
    renderable: Arc<Renderable>,
}

/// Where a node's result comes from in an evaluation, as [`Evaluated::result_mesh`] looks.
fn source(evaluated: &Evaluated, id: NodeId) -> Option<(Arc<Mesh>, Option<Xform>)> {
    if let Some(mesh) = evaluated.node_meshes.get(&id) {
        return Some((mesh.clone(), None));
    }
    let mesh = evaluated.group_meshes.get(&id)?;
    let frame = evaluated.node_frames.get(&id)?;
    Some((mesh.clone(), Some(*frame)))
}

impl RenderableCache {
    /// The renderable for `wanted`, made now if needed; `None` without a result.
    pub fn get(&self, evaluated: &Evaluated, wanted: Wanted) -> Option<Arc<Renderable>> {
        let (mesh, frame) = source(evaluated, wanted.0)?;
        if let Some(entry) = self.entries.lock().expect("the cache lock").get(&wanted) {
            if Arc::ptr_eq(&entry.mesh, &mesh) && entry.frame == frame {
                return Some(entry.renderable.clone());
            }
        }
        // Made outside the lock, since a large mesh takes a while.
        let result = evaluated.result_mesh(wanted.0)?;
        let renderable = Arc::new(match wanted.1 {
            true => Renderable::prepare_outlined(&result),
            false => Renderable::prepare(&result),
        });
        let entry = Entry { mesh, frame, renderable: renderable.clone() };
        self.entries.lock().expect("the cache lock").insert(wanted, entry);
        Some(renderable)
    }

    /// Forget every entry not in `keep`, freeing undrawn meshes.
    pub fn retain(&self, keep: &[Wanted]) {
        self.entries.lock().expect("the cache lock").retain(|wanted, _| keep.contains(wanted));
    }
}
