//! Evaluation and export off the interaction path (spec sections 2.6, 5.2, 9).
//!
//! The interface must never freeze. Both the geometry evaluation and the export
//! run on their own threads, report progress, and can be cancelled; an
//! evaluation is additionally *superseded* cleanly when the user edits again
//! while one is running -- the worker drops the stale job rather than finishing it
//! and then throwing the answer away.

use scadstudio_core::eval::{Cancel, Evaluated, Evaluator};
use scadstudio_core::scene::Scene;
use scadstudio_export::{ExportError, Options};
use scadstudio_geom::Mesh;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

struct Job {
    scene: Scene,
    cancel: Cancel,
    generation: u64,
}

pub struct Finished {
    pub result: Evaluated,
    pub generation: u64,
    pub elapsed: Duration,
}

/// Owns the evaluation thread. The `Evaluator` -- and so the whole subtree cache
/// -- lives on that thread, which is what makes a one-dimension edit cheap: only
/// the subtrees whose content hash changed are recomputed.
pub struct EvalWorker {
    jobs: Sender<Job>,
    done: Receiver<Finished>,
    current: Option<Cancel>,
    generation: u64,
    /// The generation whose result we are still waiting for.
    outstanding: Option<u64>,
    pub last_elapsed: Option<Duration>,
}

impl EvalWorker {
    pub fn spawn() -> EvalWorker {
        let (job_tx, job_rx) = mpsc::channel::<Job>();
        let (done_tx, done_rx) = mpsc::channel::<Finished>();
        std::thread::Builder::new()
            .name("scadstudio-eval".into())
            .spawn(move || evaluation_loop(job_rx, done_tx))
            .expect("the platform can start a thread");
        EvalWorker { jobs: job_tx, done: done_rx, current: None, generation: 0, outstanding: None, last_elapsed: None }
    }

    /// Ask for a fresh evaluation, cancelling whatever is in flight.
    pub fn submit(&mut self, scene: &Scene) {
        if let Some(cancel) = self.current.take() {
            cancel.cancel();
        }
        self.generation += 1;
        let cancel = Cancel::new();
        self.current = Some(cancel.clone());
        self.outstanding = Some(self.generation);
        let job = Job { scene: scene.clone(), cancel, generation: self.generation };
        // A send failure means the worker thread is gone, which we cannot
        // recover from here; the interface stays usable with the last result.
        let _ = self.jobs.send(job);
    }

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
        self.last_elapsed = Some(finished.elapsed);
        Some(finished.result)
    }

    pub fn is_busy(&self) -> bool {
        self.outstanding.is_some()
    }
}

fn evaluation_loop(jobs: Receiver<Job>, done: Sender<Finished>) {
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

/// An export in flight. Progress is a permille count in an atomic so the UI can
/// read it every frame without locking.
pub struct ExportJob {
    pub path: PathBuf,
    pub format_label: String,
    progress: Arc<AtomicU32>,
    cancelled: Arc<AtomicBool>,
    result: Receiver<Result<(), ExportError>>,
    started: Instant,
    limit: Duration,
}

impl ExportJob {
    /// `limit` is the point at which the export gives up with a clear message
    /// rather than hanging indefinitely (spec section 9).
    pub fn spawn(path: PathBuf, mesh: Arc<Mesh>, options: Options, limit: Duration) -> ExportJob {
        let progress = Arc::new(AtomicU32::new(0));
        let cancelled = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let format_label = options.format.label().to_string();

        let worker_progress = progress.clone();
        let worker_cancelled = cancelled.clone();
        let worker_path = path.clone();
        std::thread::Builder::new()
            .name("scadstudio-export".into())
            .spawn(move || {
                let deadline = Instant::now() + limit;
                let mut report = |fraction: f32| {
                    worker_progress.store((fraction.clamp(0.0, 1.0) * 1000.0) as u32, Ordering::Relaxed);
                    if Instant::now() > deadline {
                        // Treated as a cancellation by the writer, so no partial
                        // file survives; the message is corrected below.
                        return false;
                    }
                    !worker_cancelled.load(Ordering::Relaxed)
                };
                let outcome = scadstudio_export::write(&worker_path, &mesh, &options, &mut report);
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

#[cfg(test)]
mod tests {
    use super::*;
    use scadstudio_core::primitive::ParamValue;
    use scadstudio_core::scene::GroupOp;
    use scadstudio_export::Format;

    fn wait_for<T>(mut poll: impl FnMut() -> Option<T>) -> T {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(value) = poll() {
                return value;
            }
            assert!(Instant::now() < deadline, "the worker never answered");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn drilled_plate() -> Scene {
        let mut scene = Scene::new();
        let root = scene.root();
        let group = scene.add_group(GroupOp::Difference, root, 0);
        scene.add_primitive("plate", group, 0).unwrap();
        let hole = scene.add_primitive("cylinder", group, 1).unwrap();
        let params = scene.get_mut(hole).unwrap().params_mut().unwrap();
        params.insert("diameter_x".into(), ParamValue::Length(6.0));
        params.insert("diameter_y".into(), ParamValue::Length(6.0));
        params.insert("height".into(), ParamValue::Length(20.0));
        scene
    }

    #[test]
    fn an_evaluation_comes_back_from_the_worker() {
        let mut worker = EvalWorker::spawn();
        assert!(!worker.is_busy());
        worker.submit(&drilled_plate());
        assert!(worker.is_busy());
        let result = wait_for(|| worker.poll());
        assert!(result.errors.is_empty());
        assert!(result.mesh.triangle_count() > 0);
        assert!(!worker.is_busy());
        assert!(worker.last_elapsed.is_some());
    }

    #[test]
    fn a_burst_of_edits_yields_the_newest_result_and_no_stale_ones() {
        let mut worker = EvalWorker::spawn();
        let mut scene = drilled_plate();
        let hole = scene.depth_first().into_iter().last().unwrap();
        for diameter in [4.0, 5.0, 6.0, 7.0, 8.0] {
            scene
                .get_mut(hole)
                .unwrap()
                .params_mut()
                .unwrap()
                .insert("diameter_x".into(), ParamValue::Length(diameter));
            scene
                .get_mut(hole)
                .unwrap()
                .params_mut()
                .unwrap()
                .insert("diameter_y".into(), ParamValue::Length(diameter));
            worker.submit(&scene);
        }
        let result = wait_for(|| worker.poll());
        // The final 8mm hole, not one of the superseded ones. Nothing sits
        // inside the hole, so the closest vertex to its axis is on its wall --
        // a more reliable measure than the farthest, since the boolean scatters
        // T-junction vertices across the plate's faces near the hole too.
        let hole_radius = result.mesh.positions.iter().map(|p| p.x.hypot(p.y)).fold(f64::MAX, f64::min);
        assert!((hole_radius - 4.0).abs() < 1e-6, "got radius {hole_radius}, expected 4mm");
        // Nothing left queued behind it.
        assert!(worker.poll().is_none());
        assert!(!worker.is_busy());
    }

    #[test]
    fn the_cache_survives_between_submissions_so_repeat_edits_get_faster() {
        let mut worker = EvalWorker::spawn();
        let scene = drilled_plate();
        worker.submit(&scene);
        wait_for(|| worker.poll());
        let cold = worker.last_elapsed.unwrap();
        // The identical scene is a pure cache hit on the worker's own evaluator.
        worker.submit(&scene);
        wait_for(|| worker.poll());
        let warm = worker.last_elapsed.unwrap();
        assert!(warm <= cold, "a repeat evaluation took longer: {cold:?} -> {warm:?}");
    }

    #[test]
    fn an_export_reports_progress_and_finishes() {
        let mesh = Arc::new(scadstudio_geom::primitives::box_mesh(40.0, 20.0, 4.0));
        let path = std::env::temp_dir().join(format!("scadstudio-worker-{}.3mf", std::process::id()));
        let job = ExportJob::spawn(
            path.clone(),
            mesh,
            Options { format: Format::ThreeMf, ..Default::default() },
            Duration::from_secs(30),
        );
        let outcome = wait_for(|| job.poll());
        assert!(outcome.is_ok(), "{outcome:?}");
        assert_eq!(job.fraction(), 1.0);
        assert!(path.exists());
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn a_cancelled_export_reports_cancellation_and_leaves_no_file() {
        // Spec acceptance criterion 16.
        let mesh = Arc::new(scadstudio_geom::primitives::ellipsoid_mesh(40.0, 40.0, 40.0, 200));
        let path = std::env::temp_dir().join(format!("scadstudio-cancel-{}.3mf", std::process::id()));
        let job = ExportJob::spawn(
            path.clone(),
            mesh,
            Options { format: Format::ThreeMf, ..Default::default() },
            Duration::from_secs(30),
        );
        job.cancel();
        let outcome = wait_for(|| job.poll());
        // Fast machines may finish before the cancel lands; either way no
        // half-written file may survive.
        match outcome {
            Err(ExportError::Cancelled) => assert!(!path.exists(), "cancelling left a file behind"),
            Ok(()) => {
                assert!(path.exists());
                std::fs::remove_file(&path).unwrap();
            }
            other => panic!("unexpected outcome {other:?}"),
        }
    }

    #[test]
    fn an_export_that_runs_past_its_time_limit_says_so() {
        let mesh = Arc::new(scadstudio_geom::primitives::ellipsoid_mesh(40.0, 40.0, 40.0, 400));
        let path = std::env::temp_dir().join(format!("scadstudio-limit-{}.ply", std::process::id()));
        let job = ExportJob::spawn(
            path.clone(),
            mesh,
            Options { format: scadstudio_export::Format::PlyAscii, ..Default::default() },
            // Effectively zero, so the first progress callback trips it.
            Duration::from_nanos(1),
        );
        let outcome = wait_for(|| job.poll());
        match outcome {
            Err(ExportError::Io(message)) => {
                assert!(message.contains("longer than"), "{message}");
                assert!(!path.exists(), "the timed-out export left a file behind");
            }
            other => panic!("expected a time-limit message, got {other:?}"),
        }
    }
}
