//! Objects being cut in two along a face's plane (issue 130), on their own thread since each is two
//! booleans. It counts objects done, can be stopped, and leaves the document untouched until it lands.

use super::job::Task;
use simple3d_core::scene::{NodeData, NodeId};
use simple3d_geom::plane_cut::{cut_in_two, CutPlane};
use simple3d_geom::Mesh;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// One object to cut: its shape in its own frame, the plane in that frame, and the object as it was,
/// rechecked on arrival so halves never land on an edited object.
pub struct CutTarget {
    pub node: NodeId,
    pub before: NodeData,
    pub mesh: Arc<Mesh>,
    pub plane: CutPlane,
}

pub struct CutJob {
    pub tab: usize,
    pub targets: Vec<CutTarget>,
    done: Arc<AtomicU32>,
    /// Each target's two halves, in target order, `None` where the plane left it whole.
    task: Task<Option<Vec<Option<[Mesh; 2]>>>>,
}

impl CutJob {
    pub fn spawn(tab: usize, targets: Vec<CutTarget>) -> CutJob {
        let done = Arc::new(AtomicU32::new(0));
        let worker_done = done.clone();
        let work: Vec<(Arc<Mesh>, CutPlane)> = targets.iter().map(|t| (t.mesh.clone(), t.plane)).collect();
        let task = Task::spawn("simple3d-cut", move |cancelled| {
            let give_up = || cancelled.load(Ordering::Relaxed);
            let mut halves = Vec::with_capacity(work.len());
            for (mesh, plane) in &work {
                halves.push(cut_in_two(mesh, plane, &give_up));
                if give_up() {
                    return None;
                }
                worker_done.fetch_add(1, Ordering::Relaxed);
            }
            Some(halves)
        });
        CutJob { tab, targets, done, task }
    }

    /// The fraction of objects cut.
    pub fn fraction(&self) -> f32 {
        if self.targets.is_empty() {
            return 1.0;
        }
        (self.done.load(Ordering::Relaxed) as f32 / self.targets.len() as f32).clamp(0.0, 1.0)
    }

    pub fn cancel(&self) {
        self.task.cancel();
    }

    pub fn elapsed(&self) -> Duration {
        self.task.elapsed()
    }

    /// The halves once cut; the inner `None` means stopped, or a thread that died.
    pub fn poll(&self) -> Option<Option<Vec<Option<[Mesh; 2]>>>> {
        self.task.poll(|| None)
    }
}
