//! The split running on its own thread.

use simple3d_core::scene::{NodeData, NodeId};
use simple3d_geom::tiling::SplitPlan;
use simple3d_geom::Mesh;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

/// A shape being cut into a pattern of pieces (issue 82).
///
/// It is here, on a thread of its own, for the reason the evaluation is: a
/// hexagon tiling over a plate is hundreds of booleans, and an interface that
/// stops answering for a minute is one nobody can tell from a crashed one. So
/// the split reports how many cells it has finished, it can be stopped, and the
/// document is untouched until it lands.
pub struct SplitJob {
    /// The node being cut, and the document it belongs to. Both are checked
    /// again when the pieces arrive: a split must never be applied to a shape
    /// that has been edited under it, or to whatever the *other* tab happens to
    /// have selected.
    pub node: NodeId,
    pub tab: usize,
    /// The shape as it was when the cutting started, in its own frame. What the
    /// pieces are only means anything against this, so it travels with them.
    pub before: NodeData,
    pub plan: SplitPlan,
    /// How many cells will be tried, over every pass -- what the progress bar
    /// reads against.
    pub cells: usize,
    pub(super) done: Arc<AtomicU32>,
    pub(super) cancelled: Arc<AtomicBool>,
    pub(super) result: Receiver<Option<Vec<Mesh>>>,
    pub(super) started: Instant,
}

impl SplitJob {
    pub fn spawn(node: NodeId, tab: usize, before: NodeData, mesh: Arc<Mesh>, plan: SplitPlan) -> SplitJob {
        // The cells that will actually be tried, so the bar reads against the
        // number the status line quoted rather than against the arithmetic
        // bound that includes the margin around the shape. A second pass is
        // counted against the pieces the first one leaves, which is what makes
        // the bar run at one speed across the whole cutting.
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

    /// How much of the split is done, as a fraction. The cells are counted
    /// before any of them is cut, so this is honest progress rather than the
    /// spinner an evaluation has to make do with.
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

    /// The pieces, once they are cut. The inner `None` is a split that was
    /// stopped: there are no pieces, and nothing is to be changed.
    pub fn poll(&self) -> Option<Option<Vec<Mesh>>> {
        match self.result.try_recv() {
            Ok(pieces) => Some(pieces),
            Err(TryRecvError::Empty) => None,
            // The thread died, which is not something to change a document on.
            Err(TryRecvError::Disconnected) => Some(None),
        }
    }
}
