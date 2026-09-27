//! Giving up part-way through, and leaving nothing behind.

use super::*;
use crate::primitive::ParamValue;
use crate::scene::{GroupOp, Scene};
use simple3d_geom::Vec3;
use std::sync::Arc;

#[test]
pub(crate) fn a_boolean_in_flight_can_be_cancelled() {
    // Regression: `combine` took no cancellation, so a long union could not be interrupted. Five
    // overlapping 128-segment spheres cost ~640 ms here, 2.5x the floor below (four at 96 were too
    // fast and failed the precondition).
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Union, root, 0);
    for i in 0..5 {
        let id = scene.add_primitive("sphere", group, i).expect("the sphere is in the registry");
        let node = scene.get_mut(id).unwrap();
        node.segments = Some(128);
        node.position = Vec3::new(i as f64 * 12.0, 0.0, 0.0);
    }

    // The uninterrupted time on this machine, to measure the assertion against.
    let uninterrupted = std::time::Instant::now();
    let whole = Evaluator::new().evaluate(&scene, &Cancel::new());
    let uninterrupted = uninterrupted.elapsed();
    assert!(!whole.cancelled);
    assert!(
        uninterrupted > std::time::Duration::from_millis(250),
        "this test needs a union that takes a while; it took {uninterrupted:?}"
    );

    let cancel = Cancel::new();
    let flag = cancel.clone();
    let started = std::time::Instant::now();
    let runner = std::thread::spawn(move || Evaluator::new().evaluate(&scene, &cancel));
    std::thread::sleep(std::time::Duration::from_millis(60));
    flag.cancel();
    let out = runner.join().expect("the evaluation thread did not panic");
    let took = started.elapsed();

    assert!(out.cancelled, "the run does not report itself cancelled");
    assert!(took < uninterrupted / 2, "it took {took:?} of the {uninterrupted:?} it takes uninterrupted");
}

#[test]
pub(crate) fn cancelling_abandons_the_run() {
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Difference, root, 0);
    plate(&mut scene, group);
    cylinder(&mut scene, group, 6.0, 20.0);

    let cancel = Cancel::new();
    cancel.cancel();
    let out = Evaluator::new().evaluate(&scene, &cancel);
    assert!(out.cancelled);
}

#[test]
pub(crate) fn an_interrupted_evaluation_leaves_nothing_of_itself_in_the_cache() {
    // Regression: an abandoned boolean's empty mesh was cached under the scene's content hash, so the
    // shapes vanished until the hash changed ("4 nodes, 0 triangles" from a cache hit).
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Union, root, 0);
    // Heavy enough that the boolean is still running when the cancel lands.
    for i in 0..2 {
        let id = scene.add_primitive("sphere", group, i).unwrap();
        scene.get_mut(id).unwrap().position = Vec3::new(i as f64 * 12.0, 0.0, 0.0);
        scene.get_mut(id).unwrap().segments = Some(96);
        scene.get_mut(id).unwrap().params_mut().unwrap().insert("diameter".into(), ParamValue::Length(30.0));
    }
    let whole = Evaluator::new().evaluate(&scene, &Cancel::new()).mesh.triangle_count();
    assert!(whole > 0, "the scene this is about evaluates to nothing even uninterrupted");

    // One evaluator across both runs, as the worker keeps one.
    let mut evaluator = Evaluator::new();
    let cancel = Arc::new(Cancel::new());
    let flag = Arc::clone(&cancel);
    // Cancelled while the boolean runs; landing late is harmless, since the run then finishes honestly.
    let hand = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(5));
        flag.cancel();
    });
    let interrupted = evaluator.evaluate(&scene, &cancel);
    hand.join().unwrap();
    assert!(interrupted.cancelled, "the run was not interrupted, so this proves nothing");

    // The next frame: same scene, nothing cancelled, same evaluator.
    let again = evaluator.evaluate(&scene, &Cancel::new());
    assert_eq!(
        again.mesh.triangle_count(),
        whole,
        "the abandoned run left its empty mesh in the cache: the shapes are gone from every frame after it"
    );
}
