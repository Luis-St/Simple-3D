//! Sketch nodes (issue 74): the solid made from a sketch, its errors, where it is put, and its file form.

use super::*;
use crate::primitive::{ParamValue, Params};
use crate::scene::{sketch_params, Body, Placing, Scene};
use crate::xform::Xform;
use simple3d_geom::Vec3;
use simple3d_sketch::{Shape, Sketch};
use std::collections::BTreeMap;

/// A 10 x 20 rectangle at the sketch origin with its one region picked, and its left edge as an axis.
fn picked_rectangle() -> Sketch {
    let mut sketch = Sketch::default();
    let corners = [[0.0, 0.0], [10.0, 0.0], [10.0, 20.0], [0.0, 20.0]].map(|at| sketch.add_point(at));
    let lines = [0, 1, 2, 3].map(|i| sketch.add_curve(Shape::Line { a: corners[i], b: corners[(i + 1) % 4] }, false));
    let regions = simple3d_sketch::regions::find(&sketch, 64);
    sketch.regions = vec![regions.pick(0, [5.0, 5.0])];
    sketch.axis = Some(lines[3]);
    sketch
}

fn with(params: &[(&str, ParamValue)]) -> Params {
    let mut out = sketch_params(&Params::new());
    for (key, value) in params {
        out.insert(key.to_string(), *value);
    }
    out
}

fn evaluate(scene: &Scene) -> Evaluated {
    Evaluator::new().evaluate(scene, &Cancel::new())
}

#[test]
fn a_sketch_on_the_ground_extrudes_upward_and_revolves_about_its_axis() {
    let mut scene = Scene::new();
    let plane = Xform::from_translation(Vec3::new(0.0, 0.0, 5.0));
    let id = scene.add_sketch("Sketch", picked_rectangle(), with(&[]), &plane, None, &BTreeMap::new()).unwrap();
    let out = evaluate(&scene);
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert!((out.mesh.signed_volume() - 2000.0).abs() < 1e-6);
    let (lo, hi) = out.bounds.unwrap();
    assert!((lo.z - 5.0).abs() < 1e-9 && (hi.z - 15.0).abs() < 1e-9, "it did not rise from its plane");

    let Body::Sketch { sketch, .. } = scene.node(id).body.clone() else { panic!() };
    scene.set_sketch(id, (*sketch).clone(), with(&[("feature", ParamValue::Choice(1))]));
    let out = evaluate(&scene);
    let volume = out.mesh.signed_volume();
    let expected = std::f64::consts::PI * 100.0 * 20.0;
    assert!((volume - expected).abs() / expected < 0.01, "revolved to {volume}");
}

#[test]
fn a_sketch_with_nothing_picked_says_so_and_makes_nothing() {
    let mut scene = Scene::new();
    let mut sketch = picked_rectangle();
    sketch.regions.clear();
    let id =
        scene.add_sketch("Sketch", sketch, with(&[]), &Xform::from_translation(Vec3::ZERO), None, &BTreeMap::new());
    let out = evaluate(&scene);
    assert_eq!(out.errors.len(), 1);
    assert_eq!(Some(out.errors[0].node), id);
    assert_eq!(out.mesh.triangle_count(), 0);
}

#[test]
fn a_cutting_sketch_on_a_face_is_taken_out_of_the_object_under_it() {
    let mut scene = Scene::new();
    let root = scene.root();
    let block = scene.add_primitive("box", root, 0).unwrap();
    let frames = evaluate(&scene).node_frames.clone();
    let before = evaluate(&scene).mesh.signed_volume();
    // On the box's top face, cutting down into it 2 deep.
    let (lo, hi) = evaluate(&scene).bounds.unwrap();
    let plane = Xform::from_translation(Vec3::new(lo.x, lo.y, hi.z));
    let params = with(&[("distance", ParamValue::Length(2.0)), ("flip", ParamValue::Bool(true))]);
    let mut sketch = Sketch::default();
    let c = [[1.0, 1.0], [3.0, 1.0], [3.0, 3.0], [1.0, 3.0]].map(|at| sketch.add_point(at));
    for i in 0..4 {
        sketch.add_curve(Shape::Line { a: c[i], b: c[(i + 1) % 4] }, false);
    }
    sketch.regions = vec![simple3d_sketch::regions::find(&sketch, 64).pick(0, [2.0, 2.0])];
    let id = scene.add_sketch("Pocket", sketch, params, &plane, Some((block, Placing::Cut)), &frames).unwrap();
    let parent = scene.node(id).parent.unwrap();
    assert_eq!(scene.node(parent).group_op(), Some(crate::scene::GroupOp::Difference));
    let after = evaluate(&scene).mesh.signed_volume();
    assert!((before - after - 8.0).abs() < 1e-6, "cut {} out", before - after);
}

#[test]
fn a_sketch_node_survives_the_project_file_and_needs_format_six() {
    let mut scene = Scene::new();
    scene.add_sketch(
        "Sketch",
        picked_rectangle(),
        with(&[]),
        &Xform::from_translation(Vec3::ZERO),
        None,
        &BTreeMap::new(),
    );
    let text = crate::project::to_string(&scene);
    assert!(text.contains("\"format\": 6"), "written as {}", &text[..60]);
    let back = crate::project::from_str(&text).unwrap();
    let node = back.node(back.node(back.root()).children[0]);
    assert_eq!(node.sketch().map(|s| (**s).clone()), Some(picked_rectangle()));
    assert!((evaluate(&back).mesh.signed_volume() - 2000.0).abs() < 1e-6);
}
