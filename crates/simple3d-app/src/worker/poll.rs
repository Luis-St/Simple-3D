//! Taking a finished evaluation back, and abandoning one.

use super::*;
use simple3d_core::eval::{Evaluated, Evaluator};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::time::{Duration, Instant};

impl EvalWorker {
    /// The newest completed result, if one has arrived. Results from superseded
    /// generations are discarded.
    pub fn poll(&mut self) -> Option<Evaluated> {
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
            // Superseded while it was on its way back.
            return None;
        }
        self.outstanding = None;
        self.current = None;
        self.started = None;
        self.last_elapsed = Some(finished.elapsed);
        // The newest edit made while this one was running goes next, so a drag
        // keeps producing pictures for as long as it lasts.
        if let Some(scene) = self.pending.take() {
            self.start(scene);
        }
        Some(finished.result)
    }

    pub fn is_busy(&self) -> bool {
        self.outstanding.is_some()
    }

    /// How long the job in flight has been running.
    pub fn waiting_for(&self) -> Option<Duration> {
        self.started.map(|at| at.elapsed())
    }

    /// Abandon the evaluation in flight and stop waiting for it.
    ///
    /// The way out of a run that is taking longer than the user is willing to
    /// give it. The viewport keeps the last result it had -- which is a picture
    /// of an older scene, and the status bar says so -- and the next edit
    /// submits a fresh job. The worker drops the abandoned answer when it
    /// notices the flag, so nothing stale can arrive later: `poll` would refuse
    /// it on its generation anyway.
    pub fn abandon(&mut self) {
        if let Some(cancel) = self.current.take() {
            cancel.cancel();
        }
        // Including whatever was waiting behind it: the user asked to stop
        // waiting, and starting the next run on the spot is not that. The next
        // edit submits again.
        self.pending = None;
        self.outstanding = None;
        self.started = None;
    }
}

pub(super) fn evaluation_loop(jobs: Receiver<Job>, done: Sender<Finished>) {
    let mut evaluator = Evaluator::new();
    while let Ok(mut job) = jobs.recv() {
        // Skip straight to the newest queued edit: finishing a superseded one
        // would only delay the answer the user is actually waiting for.
        while let Ok(newer) = jobs.try_recv() {
            job.cancel.cancel();
            job = newer;
        }
        let started = Instant::now();
        let result = evaluator.evaluate(&job.scene, &job.cancel);
        if result.cancelled {
            continue;
        }
        if done.send(Finished { result, generation: job.generation, elapsed: started.elapsed() }).is_err() {
            return; // the application has closed
        }
    }
}
