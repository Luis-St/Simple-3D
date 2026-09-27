//! Simplification on its own thread.

use simple3d_core::mesh_data::MeshData;
use simple3d_geom::simplify::{Outcome, Simplify};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

/// A mesh being simplified (issue 106), off the UI thread because the tool is scrubbed; a run is
/// abandoned as soon as the next number is typed.
pub struct SimplifyJob {
    /// What this run was asked for, so a stale answer can be recognised.
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

    /// The simplified mesh once made; the inner `None` is an abandoned run.
    pub fn poll(&self) -> Option<Option<Outcome>> {
        match self.result.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(TryRecvError::Empty) => None,
            // A dead thread is no reason to change the document.
            Err(TryRecvError::Disconnected) => Some(None),
        }
    }
}
