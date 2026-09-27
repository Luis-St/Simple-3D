//! Taking a finished evaluation back, and abandoning one.

use super::*;
use crate::render::Renderable;
use simple3d_core::eval::{Evaluated, Evaluator};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::time::{Duration, Instant};

impl EvalWorker {
    /// The newest completed result, if any; results from superseded generations are discarded.
    pub fn poll(&mut self) -> Option<(Evaluated, Renderable)> {
        let mut newest: Option<Finished> = None;
        loop {
            match self.done.try_recv() {
                Ok(finished) => {
                    if finished.generation >= newest.as_ref().map_or(0, |f| f.generation) {
                        newest = Some(finished);
                    }
                }
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }
        let finished = newest?;
        if finished.generation < self.generation {
            // Superseded on its way back.
            return None;
        }
        self.outstanding = None;
        self.current = None;
        self.started = None;
        self.last_elapsed = Some(finished.elapsed);
        // The newest edit made meanwhile goes next, so a drag keeps producing pictures.
        if let Some(scene) = self.pending.take() {
            self.start(scene);
        }
        Some((finished.result, finished.renderable))
    }

    pub fn is_busy(&self) -> bool {
        self.outstanding.is_some()
    }

    /// How long the current job has been running.
    pub fn waiting_for(&self) -> Option<Duration> {
        self.started.map(|at| at.elapsed())
    }

    /// Abandon the evaluation in flight. The viewport keeps the last (stale, and labelled so) result,
    /// and the next edit submits afresh; `poll` would refuse the abandoned answer anyway.
    pub fn abandon(&mut self) {
        if let Some(cancel) = self.current.take() {
            cancel.cancel();
        }
        // The waiting scene is dropped too: the user asked to stop, not to start the next run.
        self.pending = None;
        self.outstanding = None;
        self.started = None;
    }
}

pub(super) fn evaluation_loop(jobs: Receiver<Job>, done: Sender<Finished>) {
    let mut evaluator = Evaluator::new();
    while let Ok(mut job) = jobs.recv() {
        // Skip to the newest queued edit, since superseded ones would only delay it.
        while let Ok(newer) = jobs.try_recv() {
            job.cancel.cancel();
            job = newer;
        }
        let started = Instant::now();
        let result = evaluator.evaluate(&job.scene, &job.cancel);
        if result.cancelled {
            continue;
        }
        let renderable = Renderable::prepare_scene(&result.mesh, &result.ranges);
        for &wanted in &job.wanted {
            job.renderables.get(&result, wanted);
        }
        let finished = Finished { result, renderable, generation: job.generation, elapsed: started.elapsed() };
        if done.send(finished).is_err() {
            return; // the application has closed
        }
    }
}
