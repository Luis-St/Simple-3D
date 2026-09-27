//! The reassembly running on its own thread.

use simple3d_core::mesh_data::MeshData;
use simple3d_geom::reassemble::{Assembly, Reassemble};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

/// A mesh being taken apart (issue 108), off the interaction path since it takes a noticeable
/// fraction of a second, and abandonable since every scrubbed number asks again.
pub struct ReassembleJob {
    /// What this run was asked, so a late answer can be checked against the numbers on screen.
    pub plan: Reassemble,
    cancelled: Arc<AtomicBool>,
    result: Receiver<Option<Assembly>>,
    started: Instant,
}

impl ReassembleJob {
    pub fn spawn(mesh: Arc<MeshData>, plan: Reassemble) -> ReassembleJob {
        let cancelled = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let worker_cancelled = cancelled.clone();
        std::thread::Builder::new()
            .name("simple3d-reassemble".into())
            .spawn(move || {
                let give_up = || worker_cancelled.load(Ordering::Relaxed);
                let _ = tx.send(simple3d_geom::reassemble::reassemble_until(&mesh.mesh, &plan, &give_up));
            })
            .expect("the platform can start a thread");
        ReassembleJob { plan, cancelled, result: rx, started: Instant::now() }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    /// The result once known; the inner `None` means abandoned.
    pub fn poll(&self) -> Option<Option<Assembly>> {
        match self.result.try_recv() {
            Ok(found) => Some(found),
            Err(TryRecvError::Empty) => None,
            // The thread died: nothing to show.
            Err(TryRecvError::Disconnected) => Some(None),
        }
    }
}
