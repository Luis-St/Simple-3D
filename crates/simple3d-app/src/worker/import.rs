//! The import running on its own thread.

use super::job::TimedTask;
use simple3d_import::{ImportError, Model};
use std::path::PathBuf;
use std::time::Duration;

/// A file being read, off the interface thread like [`super::ExportJob`], with progress the footer
/// reads without locking.
pub struct ImportJob {
    pub path: PathBuf,
    /// The document that asked, rechecked on landing so the model never goes into another tab.
    pub tab: usize,
    task: TimedTask<Result<Model, ImportError>>,
}

impl ImportJob {
    /// `limit` is when the import gives up with a message, since a bogus file can keep a parser busy.
    pub fn spawn(path: PathBuf, tab: usize, limit: Duration) -> ImportJob {
        let worker_path = path.clone();
        let task = TimedTask::spawn(
            "simple3d-import",
            limit,
            move |report| simple3d_import::read(&worker_path, report),
            // Stopping at the limit reads as a cancellation, so nothing half-read comes back.
            move |outcome| match outcome {
                Err(ImportError::Cancelled) => Err(ImportError::Io(format!(
                    "the file took longer than {} seconds to read and was given up on",
                    limit.as_secs()
                ))),
                other => other,
            },
        );
        ImportJob { path, tab, task }
    }

    pub fn fraction(&self) -> f32 {
        self.task.fraction()
    }

    pub fn cancel(&self) {
        self.task.cancel();
    }

    pub fn elapsed(&self) -> Duration {
        self.task.elapsed()
    }

    pub fn limit(&self) -> Duration {
        self.task.limit()
    }

    /// The file's own name for the model, for the node it becomes.
    pub fn stem(&self) -> String {
        self.path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .filter(|stem| !stem.trim().is_empty())
            .unwrap_or_else(|| "Imported model".to_string())
    }

    /// `Some` once the file has been read, one way or another.
    pub fn poll(&self) -> Option<Result<Model, ImportError>> {
        self.task.poll(|| Err(ImportError::Io("the import thread stopped unexpectedly".into())))
    }
}
