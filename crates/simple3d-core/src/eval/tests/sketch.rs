//! Sketch and feature nodes (issue 74): a sketch makes nothing, the features made from its regions
//! follow it, where they are put, and their file form, old and new.

use super::*;
use crate::primitive::{ParamValue, Params};
use crate::scene::{feature_params, Body, Feature, Placing, Profile, RevolveAxis, Scene};
use crate::xform::Xform;
use simple3d_geom::Vec3;
use simple3d_sketch::{CurveId, Shape, Sketch};
use std::collections::BTreeMap;

/// A 10 x 20 rectangle at the sketch origin, and its left edge.
fn rectangle() -> (Sketch, CurveId) {
    let mut sketch = Sketch::default();
    let corners = [[0.0, 0.0], [10.0, 0.0], [10.0, 20.0], [0.0, 20.0]].map(|at| sketch.add_point(at));
    let lines = [0, 1, 2, 3].map(|i| sketch.add_curve(Shape::Line { a: corners[i], b: corners[(i + 1) % 4] }, false));
    (sketch, lines[3])
}

/// The feature made from `sketch`'s one region, revolved about `axis` if given.
fn made_from(sketch: &Sketch, axis: Option<CurveId>) -> Feature {
    let regions = vec![simple3d_sketch::regions::find(sketch, 64).pick(0, [5.0, 5.0])];
    Feature { profile: Profile::Sketch { sketch: sketch.clone(), regions }, axis: axis.map(RevolveAxis::Line) }
}

fn with(revolve: bool, params: &[(&str, ParamValue)]) -> Params {
    let mut out = feature_params(revolve, &Params::new());
    for (key, value) in params {
        out.insert(key.to_string(), *value);
    }
    out
}

fn evaluate(scene: &Scene) -> Evaluated {
    Evaluator::new().evaluate(scene, &Cancel::new())
}

/// A sketch on the plane at `plane` and an extrusion of its region, linked to it.
fn sketched(scene: &mut Scene, plane: &Xform) -> (crate::scene::NodeId, crate::scene::NodeId) {
    let id = scene.add_sketch("Sketch", rectangle().0, plane);
    let sketch = (**scene.node(id).sketch().unwrap()).clone();
    let solid =
        scene.add_feature("Extrusion", made_from(&sketch, None), with(false, &[]), plane, None, &BTreeMap::new());
    (id, solid.unwrap())
}

#[test]
fn a_sketch_alone_makes_nothing_and_says_nothing() {
    let mut scene = Scene::new();
    let id = scene.add_sketch("Sketch", rectangle().0, &Xform::from_translation(Vec3::ZERO));
    let out = evaluate(&scene);
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert_eq!(out.mesh.triangle_count(), 0);
    assert!(scene.node(id).sketch().unwrap().link != 0, "a sketch put in a document has no link");
    assert!(!scene.node(id).adds(), "a sketch counts as material");
}

#[test]
fn a_feature_extrudes_up_from_its_plane_and_revolves_about_its_axis() {
    let mut scene = Scene::new();
    let plane = Xform::from_translation(Vec3::new(0.0, 0.0, 5.0));
    let (_, solid) = sketched(&mut scene, &plane);
    let out = evaluate(&scene);
    assert!(out.errors.is_empty(), "{:?}", out.errors);
    assert!((out.mesh.signed_volume() - 2000.0).abs() < 1e-6);
    let (lo, hi) = out.bounds.unwrap();
    assert!((lo.z - 5.0).abs() < 1e-9 && (hi.z - 15.0).abs() < 1e-9, "it did not rise from its plane");

    let (sketch, left) = rectangle();
    scene.get_mut(solid).unwrap().body =
        Body::Feature { feature: std::sync::Arc::new(made_from(&sketch, Some(left))), params: with(true, &[]) };
    let volume = evaluate(&scene).mesh.signed_volume();
    let expected = std::f64::consts::PI * 100.0 * 20.0;
    assert!((volume - expected).abs() / expected < 0.01, "revolved to {volume}");
}

#[test]
fn changing_a_sketch_changes_the_features_made_from_it() {
    let mut scene = Scene::new();
    let (id, _) = sketched(&mut scene, &Xform::from_translation(Vec3::ZERO));
    let other = scene.add_sketch("Other", rectangle().0, &Xform::from_translation(Vec3::new(50.0, 0.0, 0.0)));
    let mut wider = (**scene.node(id).sketch().unwrap()).clone();
    for point in wider.points.iter_mut().filter(|p| p.at[0] > 5.0) {
        point.at[0] = 20.0;
    }
    assert!(scene.set_sketch(id, wider));
    assert!((evaluate(&scene).mesh.signed_volume() - 4000.0).abs() < 1e-6, "the extrusion did not follow");
    assert_eq!(scene.features_of(scene.node(id).sketch().unwrap().link).len(), 1);
    assert!(scene.features_of(scene.node(other).sketch().unwrap().link).is_empty());
}

#[test]
fn a_feature_whose_region_is_gone_says_so_and_makes_nothing() {
    let mut scene = Scene::new();
    let (id, solid) = sketched(&mut scene, &Xform::from_translation(Vec3::ZERO));
    let mut open = (**scene.node(id).sketch().unwrap()).clone();
    let last = open.curves.last().unwrap().id;
    open.remove_curve(last);
    scene.set_sketch(id, open);
    let out = evaluate(&scene);
    assert_eq!(out.errors.len(), 1);
    assert_eq!(out.errors[0].node, solid);
    assert_eq!(out.mesh.triangle_count(), 0);
}

#[test]
fn a_cutting_feature_on_a_face_is_taken_out_of_the_object_under_it() {
    let mut scene = Scene::new();
    let root = scene.root();
    let block = scene.add_primitive("box", root, 0).unwrap();
    let frames = evaluate(&scene).node_frames.clone();
    let before = evaluate(&scene).mesh.signed_volume();
    // On the box's top face, cutting down into it 2 deep.
    let (lo, hi) = evaluate(&scene).bounds.unwrap();
    let plane = Xform::from_translation(Vec3::new(lo.x, lo.y, hi.z));
    let mut sketch = Sketch::default();
    let c = [[1.0, 1.0], [3.0, 1.0], [3.0, 3.0], [1.0, 3.0]].map(|at| sketch.add_point(at));
    for i in 0..4 {
        sketch.add_curve(Shape::Line { a: c[i], b: c[(i + 1) % 4] }, false);
    }
    let regions = vec![simple3d_sketch::regions::find(&sketch, 64).pick(0, [2.0, 2.0])];
    let feature = Feature { profile: Profile::Sketch { sketch, regions }, axis: None };
    let params = with(false, &[("distance", ParamValue::Length(2.0)), ("flip", ParamValue::Bool(true))]);
    let id = scene.add_feature("Pocket", feature, params, &plane, Some((block, Placing::Cut)), &frames).unwrap();
    let parent = scene.node(id).parent.unwrap();
    assert_eq!(scene.node(parent).group_op(), Some(crate::scene::GroupOp::Difference));
    let after = evaluate(&scene).mesh.signed_volume();
    assert!((before - after - 8.0).abs() < 1e-6, "cut {} out", before - after);
}

#[test]
fn a_sketch_and_its_feature_survive_the_project_file_still_linked() {
    let mut scene = Scene::new();
    sketched(&mut scene, &Xform::from_translation(Vec3::ZERO));
    let text = crate::project::to_string(&scene);
    assert!(text.contains("\"format\": 7"), "written as {}", &text[..60]);
    let mut back = crate::project::from_str(&text).unwrap();
    let children = back.node(back.root()).children.clone();
    let sketch = back.node(children[0]).sketch().unwrap().clone();
    assert_eq!(sketch.curves, rectangle().0.curves);
    assert_eq!(back.features_of(sketch.link), vec![children[1]]);
    assert!((evaluate(&back).mesh.signed_volume() - 2000.0).abs() < 1e-6);
    // Still linked: the sketch changed after loading reaches the feature.
    let mut taller = (*sketch).clone();
    for point in taller.points.iter_mut().filter(|p| p.at[1] > 5.0) {
        point.at[1] = 40.0;
    }
    back.set_sketch(children[0], taller);
    assert!((evaluate(&back).mesh.signed_volume() - 4000.0).abs() < 1e-6);
}

#[test]
fn a_format_six_sketch_reads_as_its_feature_in_place_and_the_sketch_beside_it() {
    let mut scene = Scene::new();
    scene.add_sketch("Lathe", rectangle().0, &Xform::from_translation(Vec3::ZERO));
    let mut file: serde_json::Value = serde_json::from_str(&crate::project::to_string(&scene)).unwrap();
    file["format"] = 6.into();
    // The old form: the picks and the axis in the sketch, the feature in the node's parameters.
    let (sketch, left) = rectangle();
    let picks = vec![simple3d_sketch::regions::find(&sketch, 64).pick(0, [5.0, 5.0])];
    let node = &mut file["root"]["children"][0];
    node["sketch"]["regions"] = serde_json::to_value(&picks).unwrap();
    node["sketch"]["axis"] = serde_json::to_value(left).unwrap();
    let params = with(true, &[("sweep", ParamValue::Angle(360.0))]);
    let mut params = serde_json::to_value(&params).unwrap();
    params["feature"] = serde_json::to_value(ParamValue::Choice(1)).unwrap();
    node["params"] = params;

    let back = crate::project::from_str(&file.to_string()).unwrap();
    let children = back.node(back.root()).children.clone();
    assert_eq!(children.len(), 2);
    let (solid, flat) = (back.node(children[0]), back.node(children[1]));
    assert_eq!(solid.name, "Lathe");
    assert!(solid.feature().is_some_and(|f| f.is_revolve()), "the revolve did not come back as a feature");
    assert!(flat.is_sketch() && !flat.visible, "the sketch is not beside it, hidden");
    let flat_sketch = flat.sketch().unwrap();
    assert!(flat_sketch.regions.is_empty() && flat_sketch.axis.is_none(), "the sketch kept its old picks");
    assert_eq!(back.features_of(flat_sketch.link), vec![children[0]]);
    let volume = evaluate(&back).mesh.signed_volume();
    let expected = std::f64::consts::PI * 100.0 * 20.0;
    assert!((volume - expected).abs() / expected < 0.01, "revolved to {volume}");
}

#[test]
fn a_pasted_sketch_and_feature_are_linked_to_each_other_and_not_to_the_originals() {
    let mut scene = Scene::new();
    let (id, _) = sketched(&mut scene, &Xform::from_translation(Vec3::ZERO));
    let root = scene.root();
    let data = scene.export_subtree(root).unwrap();
    let copy = scene.import_subtree(&data, root, 2).unwrap();
    scene.relink(&[copy]);
    let link = scene.node(id).sketch().unwrap().link;
    assert_eq!(scene.features_of(link).len(), 1, "the copy's feature still follows the original sketch");
    let pasted = scene.node(copy).children.iter().copied().find(|&c| scene.node(c).is_sketch()).unwrap();
    let pasted_link = scene.node(pasted).sketch().unwrap().link;
    assert_ne!(pasted_link, link);
    assert_eq!(scene.features_of(pasted_link).len(), 1);
}

#[test]
fn a_duplicated_feature_alone_still_follows_the_sketch_it_was_made_from() {
    let mut scene = Scene::new();
    let (id, solid) = sketched(&mut scene, &Xform::from_translation(Vec3::ZERO));
    let copy = scene.duplicate_after(solid, solid).unwrap();
    let link = scene.node(id).sketch().unwrap().link;
    assert_eq!(scene.features_of(link), vec![solid, copy]);
}
