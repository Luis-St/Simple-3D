//! Handing an evaluation to the worker thread.

use super::*;
use simple3d_core::eval::Cancel;
use simple3d_core::scene::Scene;
use std::sync::mpsc;
use std::time::Instant;

impl EvalWorker {
    pub fn spawn() -> EvalWorker {
        let (job_tx, job_rx) = mpsc::channel::<Job>();
        let (done_tx, done_rx) = mpsc::channel::<Finished>();
        std::thread::Builder::new()
            .name("simple3d-eval".into())
            .spawn(move || evaluation_loop(job_rx, done_tx))
            .expect("the platform can start a thread");
        EvalWorker {
            jobs: job_tx,
            done: done_rx,
            current: None,
            generation: 0,
            outstanding: None,
            pending: None,
            started: None,
            last_elapsed: None,
        }
    }

    /// Ask for a fresh evaluation of the document being edited.
    ///
    /// While a run is in flight the new scene *waits* for it rather than
    /// killing it, and only the newest waiting scene is kept. Cancelling
    /// instead is what froze the viewport during a drag: a drag marks the scene
    /// dirty on every frame, so every run was cancelled by the next frame a few
    /// milliseconds in, and on the frames where the boolean actually costs
    /// something -- which is exactly when the shape being dragged meets another
    /// one -- no evaluation ever finished. The user saw 139 frames in a row
    /// with no preview at all, and then a jump when the drag stopped.
    ///
    /// Waiting costs the preview one evaluation of lag. Cancelling costs the
    /// preview altogether, and throws the work away as well. A run that really
    /// is taking too long is the user's to abandon, which the footer offers for
    /// as long as one is going.
    pub fn submit(&mut self, scene: &Scene) {
        if self.outstanding.is_some() {
            self.pending = Some(scene.clone());
            return;
        }
        self.start(scene.clone());
    }

    /// Ask for an evaluation of a *different document* -- a tab shown, a file
    /// opened -- which supersedes whatever is running.
    ///
    /// Nothing about the scene being left is worth waiting for, and its answer
    /// must never be applied to the document now on screen; the generation bump
    /// is what makes `poll` refuse it if it arrives anyway.
    pub fn supersede(&mut self, scene: &Scene) {
        if let Some(cancel) = self.current.take() {
            cancel.cancel();
        }
        self.pending = None;
        self.start(scene.clone());
    }

    pub(super) fn start(&mut self, scene: Scene) {
        self.generation += 1;
        let cancel = Cancel::new();
        self.current = Some(cancel.clone());
        self.outstanding = Some(self.generation);
        self.started = Some(Instant::now());
        let job = Job { scene, cancel, generation: self.generation };
        // A send failure means the worker thread is gone, which we cannot
        // recover from here; the interface stays usable with the last result.
        let _ = self.jobs.send(job);
    }
}
