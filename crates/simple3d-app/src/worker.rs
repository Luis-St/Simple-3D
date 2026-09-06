//! Evaluation and export off the interaction path (spec sections 2.6, 5.2, 9).
//!
//! The interface must never freeze. Both the geometry evaluation and the export
//! run on their own threads, report progress, and can be cancelled; an
//! evaluation is additionally *superseded* cleanly when the user edits again
//! while one is running -- the worker drops the stale job rather than finishing it
//! and then throwing the answer away.

use simple3d_core::eval::{Cancel, Evaluated, Evaluator};
use simple3d_core::scene::Scene;
use simple3d_export::{ExportError, Options};
use simple3d_geom::Mesh;
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
    /// The newest scene, waiting for the run in flight to finish.
    ///
    /// Only ever one: a drag submits on every frame, and what the viewport owes
    /// the user is the newest of those, not each of them.
    pending: Option<Scene>,
    /// When the job in flight was submitted, so the footer can say how long the
    /// user has been waiting. An evaluation has no honest progress to report --
    /// a boolean does not know how much of itself is left -- but it can always
    /// say how long it has been going.
    started: Option<Instant>,
    pub last_elapsed: Option<Duration>,
}

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

    fn start(&mut self, scene: Scene) {
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
    /// Each pair is one object of the export and the name it is written under;
    /// a single-body export is one part with no name of its own. Whether
    /// several of them become separate components or are merged into one body
    /// is `options.bodies`, which the writer applies (issue 58).
    ///
    /// `limit` is the point at which the export gives up with a clear message
    /// rather than hanging indefinitely (spec section 9).
    pub fn spawn_parts(path: PathBuf, parts: Vec<(String, Arc<Mesh>)>, options: Options, limit: Duration) -> ExportJob {
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
                        // Treated as a cancellation by the writer, so no partial
                        // file survives; the message is corrected below.
                        return false;
                    }
                    !worker_cancelled.load(Ordering::Relaxed)
                };
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

#[cfg(test)]
mod tests {
    use super::*;
    use simple3d_core::primitive::ParamValue;
    use simple3d_core::scene::GroupOp;
    use simple3d_export::Format;

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

    /// The radius of the hole in a drilled plate. Nothing sits inside the hole,
    /// so the closest vertex to its axis is on its wall -- a more reliable
    /// measure than the farthest, since the boolean scatters T-junction
    /// vertices across the plate's faces near the hole too.
    fn hole_radius(result: &Evaluated) -> f64 {
        result.mesh.positions.iter().map(|p| p.x.hypot(p.y)).fold(f64::MAX, f64::min)
    }

    fn burst(worker: &mut EvalWorker, diameters: [f64; 5]) {
        let mut scene = drilled_plate();
        let hole = scene.depth_first().into_iter().last().unwrap();
        for diameter in diameters {
            let params = scene.get_mut(hole).unwrap().params_mut().unwrap();
            params.insert("diameter_x".into(), ParamValue::Length(diameter));
            params.insert("diameter_y".into(), ParamValue::Length(diameter));
            worker.submit(&scene);
        }
    }

    #[test]
    fn a_burst_of_edits_keeps_answering_and_settles_on_the_newest() {
        // What a drag is: an edit on every frame, faster than the evaluation
        // they ask for. Each one used to cancel the run in flight, so while the
        // scene was expensive enough to matter -- a shape being dragged into
        // another one, where the union stops being a bounding-box rejection and
        // becomes a real boolean -- nothing ever finished and the viewport
        // showed the same picture for the whole gesture.
        //
        // Now the newest edit waits, so answers keep arriving; the last of them
        // is the newest scene, which is the part that was never negotiable.
        let mut worker = EvalWorker::spawn();
        burst(&mut worker, [4.0, 5.0, 6.0, 7.0, 8.0]);

        let mut answers: Vec<f64> = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(20);
        while worker.is_busy() {
            if let Some(result) = worker.poll() {
                answers.push(hole_radius(&result));
            }
            assert!(Instant::now() < deadline, "the worker never finished the burst");
            std::thread::sleep(Duration::from_millis(1));
        }
        // Answers, plural: a burst that produces one picture is a frozen
        // viewport, whatever it settles on afterwards.
        assert!(answers.len() >= 2, "the whole burst produced one answer: {answers:?}");
        // The final 8mm hole, not one of the ones it overtook.
        let last = *answers.last().unwrap();
        assert!((last - 4.0).abs() < 1e-6, "settled on radius {last}, expected 4mm");
        // Nothing left queued behind it.
        assert!(worker.poll().is_none());
        assert!(!worker.is_busy());
    }

    #[test]
    fn a_document_shown_supersedes_the_evaluation_of_the_one_left_behind() {
        // A tab switch is the one submission that must not wait: the answer the
        // old document is still working on would be applied to the new one.
        let mut worker = EvalWorker::spawn();
        burst(&mut worker, [4.0, 5.0, 6.0, 7.0, 8.0]);
        let mut other = drilled_plate();
        let hole = other.depth_first().into_iter().last().unwrap();
        let params = other.get_mut(hole).unwrap().params_mut().unwrap();
        params.insert("diameter_x".into(), ParamValue::Length(12.0));
        params.insert("diameter_y".into(), ParamValue::Length(12.0));
        worker.supersede(&other);

        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(result) = worker.poll() {
                let radius = hole_radius(&result);
                assert!((radius - 6.0).abs() < 1e-6, "the document left behind was drawn: radius {radius}");
                break;
            }
            assert!(Instant::now() < deadline, "the worker never answered");
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(!worker.is_busy(), "the superseded edits are still queued");
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
        let mesh = Arc::new(simple3d_geom::primitives::box_mesh(40.0, 20.0, 4.0));
        let path = std::env::temp_dir().join(format!("simple3d-worker-{}.3mf", std::process::id()));
        let job = ExportJob::spawn_parts(
            path.clone(),
            vec![(String::new(), mesh)],
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
        let mesh = Arc::new(simple3d_geom::primitives::ellipsoid_mesh(40.0, 40.0, 40.0, 200));
        let path = std::env::temp_dir().join(format!("simple3d-cancel-{}.3mf", std::process::id()));
        let job = ExportJob::spawn_parts(
            path.clone(),
            vec![(String::new(), mesh)],
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
        let mesh = Arc::new(simple3d_geom::primitives::ellipsoid_mesh(40.0, 40.0, 40.0, 400));
        let path = std::env::temp_dir().join(format!("simple3d-limit-{}.ply", std::process::id()));
        let job = ExportJob::spawn_parts(
            path.clone(),
            vec![(String::new(), mesh)],
            Options { format: simple3d_export::Format::PlyAscii, ..Default::default() },
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
