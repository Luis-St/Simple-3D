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
            renderables: RenderableCache::default(),
            wanted: Vec::new(),
        }
    }

    /// Declare which single nodes the viewport draws, so every evaluation prepares them first.
    pub fn want(&mut self, wanted: Vec<Wanted>) {
        self.wanted = wanted;
    }

    /// Request an evaluation of the edited document. During a run the new scene waits (only the newest
    /// kept) rather than cancelling it: cancelling every frame during a drag meant no evaluation ever
    /// finished when shapes met. Long runs can be stopped from the footer.
    pub fn submit(&mut self, scene: &Scene) {
        if self.outstanding.is_some() {
            self.pending = Some(scene.clone());
            return;
        }
        self.start(scene.clone());
    }

    /// Request an evaluation of a different document, superseding the current run; the generation bump
    /// makes `poll` refuse a stale answer.
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
        let job = Job {
            scene,
            cancel,
            generation: self.generation,
            wanted: self.wanted.clone(),
            renderables: self.renderables.clone(),
        };
        // A send failure means the worker is gone; the interface keeps the last result.
        let _ = self.jobs.send(job);
    }
}
