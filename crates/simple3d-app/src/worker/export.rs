//! The export running on its own thread.

use super::job::TimedTask;
use simple3d_export::{ExportError, Options};
use simple3d_geom::Mesh;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// An export in flight, with progress the UI reads without locking.
pub struct ExportJob {
    pub path: PathBuf,
    pub format_label: String,
    task: TimedTask<Result<(), ExportError>>,
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
        let format_label = options.format.label().to_string();
        let worker_path = path.clone();
        let task = TimedTask::spawn(
            "simple3d-export",
            limit,
            move |report| {
                let parts = build();
                let borrowed: Vec<simple3d_export::Part<'_>> =
                    parts.iter().map(|(name, mesh)| simple3d_export::Part { name, mesh }).collect();
                simple3d_export::write_parts(&worker_path, &borrowed, &options, report)
            },
            // Stopping at the limit reads as a cancellation, so no partial file survives.
            move |outcome| match outcome {
                Err(ExportError::Cancelled) => Err(ExportError::Io(format!(
                    "the export took longer than {} seconds and was stopped; no file was written",
                    limit.as_secs()
                ))),
                other => other,
            },
        );
        ExportJob { path, format_label, task }
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

    /// `Some` once the export has finished, one way or another.
    pub fn poll(&self) -> Option<Result<(), ExportError>> {
        self.task.poll(|| Err(ExportError::Io("the export thread stopped unexpectedly".into())))
    }
}
