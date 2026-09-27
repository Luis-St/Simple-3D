//! The export running on its own thread.

use simple3d_export::{ExportError, Options};
use simple3d_geom::Mesh;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

/// An export in flight; progress is a permille atomic the UI reads without locking.
pub struct ExportJob {
    pub path: PathBuf,
    pub format_label: String,
    pub(super) progress: Arc<AtomicU32>,
    pub(super) cancelled: Arc<AtomicBool>,
    pub(super) result: Receiver<Result<(), ExportError>>,
    pub(super) started: Instant,
    pub(super) limit: Duration,
}

impl ExportJob {
    /// Export `parts` (object, name); a single body is one unnamed part, and `options.bodies` decides
    /// merging (issue 58). `limit` is when it gives up with a message rather than hanging (spec section 9).
    #[cfg(test)]
    pub fn spawn_parts(path: PathBuf, parts: Vec<(String, Arc<Mesh>)>, options: Options, limit: Duration) -> ExportJob {
        ExportJob::spawn_building(path, move || parts, options, limit)
    }

    /// [`ExportJob::spawn_parts`], computing the parts on the export thread first, since evaluating
    /// every body froze the window on large models (issue 111).
    pub fn spawn_building(
        path: PathBuf,
        build: impl FnOnce() -> Vec<(String, Arc<Mesh>)> + Send + 'static,
        options: Options,
        limit: Duration,
    ) -> ExportJob {
        let progress = Arc::new(AtomicU32::new(0));
        let cancelled = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let format_label = options.format.label().to_string();

        let worker_progress = progress.clone();
        let worker_cancelled = cancelled.clone();
        let worker_path = path.clone();
        std::thread::Builder::new()
            .name("simple3d-export".into())
            .spawn(move || {
                let deadline = Instant::now() + limit;
                let mut report = |fraction: f32| {
                    worker_progress.store((fraction.clamp(0.0, 1.0) * 1000.0) as u32, Ordering::Relaxed);
                    if Instant::now() > deadline {
                        // Treated as a cancellation, so no partial file survives; the message is corrected below.
                        return false;
                    }
                    !worker_cancelled.load(Ordering::Relaxed)
                };
                let parts = build();
                let borrowed: Vec<simple3d_export::Part<'_>> =
                    parts.iter().map(|(name, mesh)| simple3d_export::Part { name, mesh }).collect();
                let outcome = simple3d_export::write_parts(&worker_path, &borrowed, &options, &mut report);
                let outcome = match outcome {
                    Err(ExportError::Cancelled) if Instant::now() > deadline => Err(ExportError::Io(format!(
                        "the export took longer than {} seconds and was stopped; no file was written",
                        limit.as_secs()
                    ))),
                    other => other,
                };
                let _ = tx.send(outcome);
            })
            .expect("the platform can start a thread");

        ExportJob { path, format_label, progress, cancelled, result: rx, started: Instant::now(), limit }
    }

    pub fn fraction(&self) -> f32 {
        self.progress.load(Ordering::Relaxed) as f32 / 1000.0
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    pub fn limit(&self) -> Duration {
        self.limit
    }

    /// `Some` once the export has finished, one way or another.
    pub fn poll(&self) -> Option<Result<(), ExportError>> {
        match self.result.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                Some(Err(ExportError::Io("the export thread stopped unexpectedly".into())))
            }
        }
    }
}
