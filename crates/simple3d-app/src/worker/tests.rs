use super::export::*;
use simple3d_core::eval::Evaluated;
use simple3d_core::scene::Scene;
use simple3d_export::{ExportError, Options};
use std::sync::Arc;
use std::time::{Duration, Instant};

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
