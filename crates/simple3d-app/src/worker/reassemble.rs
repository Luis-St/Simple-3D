//! The reassembly running on its own thread.

use simple3d_core::mesh_data::MeshData;
use simple3d_geom::reassemble::{Assembly, Reassemble};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

/// A mesh being taken apart (issue 108).
///
/// Here rather than on the interaction path for the reason the simplification
/// is: separating a mesh of a hundred thousand triangles into its bodies and
/// fitting a shape to each of them is a fraction of a second, and it is a
/// fraction of a second the window would not be answering in -- while the
/// window is being *scrubbed*. Every change of a number asks for a new answer,
/// so a run is also something that has to be abandoned: the answer to the
/// number before last is of no interest the moment the next one is typed.
pub struct ReassembleJob {
    /// What this run was asked for, so the tool can tell whether the answer
    /// that lands is still the answer to the question on screen.
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

    /// What the mesh was found to be made of, once it is known. The inner
    /// `None` is a run that was abandoned: there is no answer, and nothing is
    /// to be shown.
    pub fn poll(&self) -> Option<Option<Assembly>> {
        match self.result.try_recv() {
            Ok(found) => Some(found),
            Err(TryRecvError::Empty) => None,
            // The thread died, which is not something to change a document on.
            Err(TryRecvError::Disconnected) => Some(None),
        }
    }
}
