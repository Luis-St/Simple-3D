//! The split running on its own thread.

use simple3d_core::scene::{NodeData, NodeId};
use simple3d_geom::tiling::SplitPlan;
use simple3d_geom::Mesh;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

/// A shape being cut into pieces (issue 82), on its own thread since it can be hundreds of
/// booleans. It reports progress, can be stopped, and leaves the document untouched until it lands.
pub struct SplitJob {
    /// The node and its document, both rechecked on arrival so the pieces never land on an edited
    /// shape or another tab.
    pub node: NodeId,
    pub tab: usize,
    /// The shape as it was when cutting started, in its own frame, which the pieces are relative to.
    pub before: NodeData,
    pub plan: SplitPlan,
    /// How many cells will be tried over every pass, for the progress bar.
    pub cells: usize,
    pub(super) done: Arc<AtomicU32>,
    pub(super) cancelled: Arc<AtomicBool>,
    pub(super) result: Receiver<Option<Vec<Mesh>>>,
    pub(super) started: Instant,
}

impl SplitJob {
    pub fn spawn(node: NodeId, tab: usize, before: NodeData, mesh: Arc<Mesh>, plan: SplitPlan) -> SplitJob {
        // Cells actually tried (later passes counted against earlier pieces), so the bar matches the
        // status line and runs evenly.
        let cells = mesh.bounds().map_or(0, |bounds| plan.work(bounds));
        let done = Arc::new(AtomicU32::new(0));
        let cancelled = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();

        let worker_done = done.clone();
        let worker_cancelled = cancelled.clone();
        let plan_for_worker = plan.clone();
        std::thread::Builder::new()
            .name("simple3d-split".into())
            .spawn(move || {
                let report = || {
                    worker_done.fetch_add(1, Ordering::Relaxed);
                };
                let give_up = || worker_cancelled.load(Ordering::Relaxed);
                let _ = tx.send(simple3d_geom::tiling::cut_plan(&mesh, &plan_for_worker, &report, &give_up));
            })
            .expect("the platform can start a thread");

        SplitJob { node, tab, before, plan, cells, done, cancelled, result: rx, started: Instant::now() }
    }

    /// The fraction done: real progress, since cells are counted up front.
    pub fn fraction(&self) -> f32 {
        if self.cells == 0 {
            return 0.0;
        }
        (self.done.load(Ordering::Relaxed) as f32 / self.cells as f32).clamp(0.0, 1.0)
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    /// The pieces once cut; the inner `None` means stopped, with nothing to change.
    pub fn poll(&self) -> Option<Option<Vec<Mesh>>> {
        match self.result.try_recv() {
            Ok(pieces) => Some(pieces),
            Err(TryRecvError::Empty) => None,
            // The thread died: nothing to change the document with.
            Err(TryRecvError::Disconnected) => Some(None),
        }
    }
}
