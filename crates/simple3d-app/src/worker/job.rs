//! What every background task shares: its thread, a cancel flag, the result channel and a clock.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

/// Work on its own named thread, told to stop through a flag it reads.
pub(crate) struct Task<T> {
    cancelled: Arc<AtomicBool>,
    result: Receiver<T>,
    started: Instant,
}

impl<T: Send + 'static> Task<T> {
    /// Run `work` on a thread called `name`; it is handed the cancel flag.
    pub(super) fn spawn(name: &str, work: impl FnOnce(&AtomicBool) -> T + Send + 'static) -> Task<T> {
        let cancelled = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let worker_cancelled = cancelled.clone();
        std::thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                let _ = tx.send(work(&worker_cancelled));
            })
            .expect("the platform can start a thread");
        Task { cancelled, result: rx, started: Instant::now() }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    /// `Some` once the work has finished; `on_dead` answers for a thread that died without a result.
    pub(super) fn poll(&self, on_dead: impl FnOnce() -> T) -> Option<T> {
        match self.result.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(on_dead()),
        }
    }
}

/// A [`Task`] reporting progress as a permille the interface reads without locking, and given up on
/// once it runs past `limit`.
pub(crate) struct TimedTask<T> {
    task: Task<T>,
    progress: Arc<AtomicU32>,
    limit: Duration,
}

impl<T: Send + 'static> TimedTask<T> {
    /// Run `work` with a progress callback that returns false once cancelled or past the limit. A run
    /// that ended after the limit has its outcome passed through `overran`, to say why it stopped.
    pub(super) fn spawn(
        name: &str,
        limit: Duration,
        work: impl FnOnce(&mut dyn FnMut(f32) -> bool) -> T + Send + 'static,
        overran: impl FnOnce(T) -> T + Send + 'static,
    ) -> TimedTask<T> {
        let progress = Arc::new(AtomicU32::new(0));
        let worker_progress = progress.clone();
        let task = Task::spawn(name, move |cancelled| {
            let deadline = Instant::now() + limit;
            let mut report = |fraction: f32| {
                worker_progress.store((fraction.clamp(0.0, 1.0) * 1000.0) as u32, Ordering::Relaxed);
                if Instant::now() > deadline {
                    return false;
                }
                !cancelled.load(Ordering::Relaxed)
            };
            let outcome = work(&mut report);
            if Instant::now() > deadline {
                overran(outcome)
            } else {
                outcome
            }
        });
        TimedTask { task, progress, limit }
    }

    pub fn fraction(&self) -> f32 {
        self.progress.load(Ordering::Relaxed) as f32 / 1000.0
    }

    pub fn cancel(&self) {
        self.task.cancel();
    }

    pub fn elapsed(&self) -> Duration {
        self.task.elapsed()
    }

    pub fn limit(&self) -> Duration {
        self.limit
    }

    pub(super) fn poll(&self, on_dead: impl FnOnce() -> T) -> Option<T> {
        self.task.poll(on_dead)
    }
}

/// A scrubbed tool's run, with the plan it answers so a late answer can be checked against the
/// numbers on screen. The result is `None` when abandoned.
pub struct PlannedJob<P, R> {
    pub plan: P,
    task: Task<Option<R>>,
}

impl<P: Copy + Send + 'static, R: Send + 'static> PlannedJob<P, R> {
    pub(super) fn spawn(
        name: &str,
        plan: P,
        work: impl FnOnce(&P, &dyn Fn() -> bool) -> Option<R> + Send + 'static,
    ) -> PlannedJob<P, R> {
        let task = Task::spawn(name, move |cancelled| work(&plan, &|| cancelled.load(Ordering::Relaxed)));
        PlannedJob { plan, task }
    }

    pub fn cancel(&self) {
        self.task.cancel();
    }

    pub fn elapsed(&self) -> Duration {
        self.task.elapsed()
    }

    /// The result once known; the inner `None` means abandoned, or a thread that died.
    pub fn poll(&self) -> Option<Option<R>> {
        self.task.poll(|| None)
    }

    /// Whether a run for `plan` may start: nothing is in flight. One at a time, since a scrub asks
    /// every frame; a run for other numbers is stopped so the next frame starts the newest.
    pub fn idle_for(job: Option<&PlannedJob<P, R>>, plan: &P) -> bool
    where
        P: PartialEq,
    {
        match job {
            Some(job) if job.plan == *plan => false,
            Some(job) => {
                job.cancel();
                false
            }
            None => true,
        }
    }
}
