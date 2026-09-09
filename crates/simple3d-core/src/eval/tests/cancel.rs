//! Giving up part-way through, and leaving nothing behind.

use super::*;
use crate::primitive::ParamValue;
use crate::scene::{GroupOp, Scene};
use simple3d_geom::Vec3;
use std::sync::Arc;

#[test]
pub(crate) fn a_boolean_in_flight_can_be_cancelled() {
    // The flag used to be checked where the work is not: `combine` took no
    // cancellation at all, so once a union started, the flag the interface
    // sets on the next edit could never land. The job ran to the end, every
    // newer edit queued behind it, and with a big enough union the
    // application could not be got out of it -- the footer still said
    // "Evaluating..." with every node deleted.
    //
    // A union of finely tessellated spheres that all overlap is the easiest
    // way to reach a boolean that costs seconds. Cancelled a moment in, it
    // must stop in a moment rather than run to the end.
    //
    // Five spheres at 128 segments, rather than the four at 96 this started
    // with: those cost 240 ms here and the floor below wants 250, so the test
    // failed on its own precondition without ever reaching the cancellation.
    // Measured on the same machine, the fixture now costs ~640 ms, which is
    // 2.5x the floor -- room for a faster machine before it needs raising
    // again.
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Union, root, 0);
    for i in 0..5 {
        let id = scene.add_primitive("sphere", group, i).expect("the sphere is in the registry");
        let node = scene.get_mut(id).unwrap();
        node.segments = Some(128);
        node.position = Vec3::new(i as f64 * 12.0, 0.0, 0.0);
    }

    // How long it takes when nobody interrupts it, so the assertion below is
    // measured against this machine rather than against a guess.
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
    // The bug this holds back, seen in the running application: three boxes
    // in a union group, all of them visible, and a viewport with nothing in
    // it -- "4 nodes, 0 triangles" in 0.03 ms, which is the time a cache hit
    // takes and not the time a boolean takes.
    //
    // An abandoned boolean returns an empty mesh (`evaluate_boolean_until`
    // gives back `Mesh::new()` the moment `give_up` says so), and the
    // worker's `Evaluator` -- and so its subtree cache -- lives for as long
    // as the application does. Cached under the content hash of a perfectly
    // good scene, that empty mesh was what every later evaluation of the
    // same content got back: the shapes vanished the moment an edit landed
    // while a boolean was running, and stayed gone until something changed
    // the hash again.
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

    // One evaluator across both runs, exactly as the worker keeps one.
    let mut evaluator = Evaluator::new();
    let cancel = Arc::new(Cancel::new());
    let flag = Arc::clone(&cancel);
    // Interrupted after the run has started and while the boolean is in it,
    // which is where a newer edit interrupts one. Landing late is harmless:
    // the run then finishes honestly and the assertion below still holds.
    let hand = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(5));
        flag.cancel();
    });
    let interrupted = evaluator.evaluate(&scene, &cancel);
    hand.join().unwrap();
    assert!(interrupted.cancelled, "the run was not interrupted, so this proves nothing");

    // The next frame: the same scene, nothing cancelled, the same evaluator.
    let again = evaluator.evaluate(&scene, &Cancel::new());
    assert_eq!(
        again.mesh.triangle_count(),
        whole,
        "the abandoned run left its empty mesh in the cache: the shapes are gone from every frame after it"
    );
}
