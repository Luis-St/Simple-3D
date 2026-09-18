//! The import running on its own thread.

use simple3d_import::{ImportError, Model};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

/// A file being read in. Shaped like [`super::ExportJob`], and for the same
/// reason: a hundred-megabyte STL is a second or two of parsing, and the
/// interface must not stop answering for it. Progress is a permille count in
/// an atomic so the footer can read it every frame without locking.
pub struct ImportJob {
    pub path: PathBuf,
    /// Which document asked for the file. Checked again when the model lands:
    /// an import must never be dropped into whatever tab the user has switched
    /// to in the meantime.
    pub tab: usize,
    pub(super) progress: Arc<AtomicU32>,
    pub(super) cancelled: Arc<AtomicBool>,
    pub(super) result: Receiver<Result<Model, ImportError>>,
    pub(super) started: Instant,
    pub(super) limit: Duration,
}

impl ImportJob {
    /// `limit` is the point at which the import gives up with a clear message
    /// rather than reading forever -- the same guard the export has, since a
    /// file that is not what it claims to be can keep a parser busy for a very
    /// long time.
    pub fn spawn(path: PathBuf, tab: usize, limit: Duration) -> ImportJob {
        let progress = Arc::new(AtomicU32::new(0));
        let cancelled = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();

        let worker_progress = progress.clone();
        let worker_cancelled = cancelled.clone();
        let worker_path = path.clone();
        std::thread::Builder::new()
            .name("simple3d-import".into())
            .spawn(move || {
                let deadline = Instant::now() + limit;
                let mut report = |fraction: f32| {
                    worker_progress.store((fraction.clamp(0.0, 1.0) * 1000.0) as u32, Ordering::Relaxed);
                    if Instant::now() > deadline {
                        // Read by the parser as a cancellation, so nothing
                        // half-read comes back; the message is corrected below.
                        return false;
                    }
                    !worker_cancelled.load(Ordering::Relaxed)
                };
                let outcome = simple3d_import::read(&worker_path, &mut report);
                let outcome = match outcome {
                    Err(ImportError::Cancelled) if Instant::now() > deadline => Err(ImportError::Io(format!(
                        "the file took longer than {} seconds to read and was given up on",
                        limit.as_secs()
                    ))),
                    other => other,
                };
                let _ = tx.send(outcome);
            })
            .expect("the platform can start a thread");

        ImportJob { path, tab, progress, cancelled, result: rx, started: Instant::now(), limit }
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

    /// The name the file itself gives the model, for the node it becomes.
    pub fn stem(&self) -> String {
        self.path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .filter(|stem| !stem.trim().is_empty())
            .unwrap_or_else(|| "Imported model".to_string())
    }

    /// `Some` once the file has been read, one way or another.
    pub fn poll(&self) -> Option<Result<Model, ImportError>> {
        match self.result.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                Some(Err(ImportError::Io("the import thread stopped unexpectedly".into())))
            }
        }
    }
}
