//! The simplification running on its own thread.

use simple3d_core::mesh_data::MeshData;
use simple3d_geom::simplify::{Outcome, Simplify};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

/// A mesh being simplified (issue 106).
///
/// Here rather than on the interaction path for the reason the evaluation and
/// the split are: a hundred thousand triangles is a fraction of a second, which
/// is a fraction of a second the window would not be answering in -- and the
/// window is being *scrubbed*. The tool asks for a run on every change of a
/// number, so a run is also something that has to be abandoned: the answer to
/// the number before last is of no interest the moment the next one is typed.
pub struct SimplifyJob {
    /// What this run was asked for, so the tool can tell whether the answer
    /// that lands is still the answer to the question on screen.
    pub plan: Simplify,
    cancelled: Arc<AtomicBool>,
    result: Receiver<Option<Outcome>>,
    started: Instant,
}

impl SimplifyJob {
    pub fn spawn(mesh: Arc<MeshData>, plan: Simplify) -> SimplifyJob {
        let cancelled = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let worker_cancelled = cancelled.clone();
        std::thread::Builder::new()
            .name("simple3d-simplify".into())
            .spawn(move || {
                let give_up = || worker_cancelled.load(Ordering::Relaxed);
                let _ = tx.send(simple3d_geom::simplify::simplify_until(&mesh.mesh, &plan, &give_up));
            })
            .expect("the platform can start a thread");
        SimplifyJob { plan, cancelled, result: rx, started: Instant::now() }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    /// The simplified mesh, once it is made. The inner `None` is a run that was
    /// abandoned: there is no mesh, and nothing is to be shown.
    pub fn poll(&self) -> Option<Option<Outcome>> {
        match self.result.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(TryRecvError::Empty) => None,
            // The thread died, which is not something to change a document on.
            Err(TryRecvError::Disconnected) => Some(None),
        }
    }
}
