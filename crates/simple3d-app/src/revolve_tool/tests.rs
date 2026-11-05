//! Sketch regions pushed and pulled, and profiles revolved (issue 74), through the tools' own hit
//! tests and commits.

use super::*;
use crate::app::tests::two_boxes;
use crate::app::App;
use crate::view::View;
use simple3d_core::xform::Xform;
use simple3d_sketch::{Shape, Sketch};

/// A rectangle from `lo` to `hi` in a sketch's plane.
fn rectangle(lo: [f64; 2], hi: [f64; 2]) -> Sketch {
    let mut sketch = Sketch::default();
    let corners = [lo, [hi[0], lo[1]], hi, [lo[0], hi[1]]].map(|at| sketch.add_point(at));
    for i in 0..4 {
        sketch.add_curve(Shape::Line { a: corners[i], b: corners[(i + 1) % 4] }, false);
    }
    sketch
}

/// Two boxes far apart, and a sketch of `sketch` on `plane`, evaluated.
fn with_sketch(tag: &str, sketch: Sketch, plane: Xform) -> (App, simple3d_core::scene::NodeId, View) {
    let (mut app, ..) = two_boxes(tag, Vec3::new(400.0, 0.0, 0.0));
    let id = app.scene.add_sketch("Sketch", sketch, &plane);
    app.reevaluate_for_test();
    let view = View::new(app.scene.camera, app.viewport_rect);
    (app, id, view)
}

fn screen(view: &View, p: Vec3) -> egui::Pos2 {
    view.project(p).expect("in front of the camera").0
}

fn volume(app: &mut App) -> f64 {
    app.reevaluate_for_test();
    app.evaluated.mesh.signed_volume()
}

#[test]
fn a_sketch_region_on_nothing_pushes_out_either_side_into_a_linked_feature_and_hides_the_sketch() {
    let plane = Xform::from_translation(Vec3::new(-200.0, 0.0, 0.0));
    let (mut app, id, view) = with_sketch("revolve-push", rectangle([0.0, 0.0], [10.0, 20.0]), plane);
    let before = volume(&mut app);
    let hit =
        app.region_under(&view, screen(&view, Vec3::new(-195.0, 10.0, 0.0))).expect("no region under the pointer");
    assert_eq!(hit.node, id);
    assert_eq!(app.object_under_region(&hit), None, "a sketch on the ground lies on an object");
    crate::push_pull_tool::region_commit(&mut app, &hit, None, -5.0);
    let solid = app.primary().unwrap();
    assert!(app.scene.node(solid).is_feature());
    assert!(!app.scene.node(id).visible, "the sketch was left shown over its solid");
    assert!((volume(&mut app) - before - 1000.0).abs() < 1e-6, "pulling a free region did not add below it");
    let (lo, _) = app.evaluated.node_world_bounds[&solid];
    assert!((lo.z + 5.0).abs() < 1e-6, "it did not run down from the plane");
    // It follows the sketch.
    let mut taller = (**app.scene.node(id).sketch().unwrap()).clone();
    for point in taller.points.iter_mut().filter(|p| p.at[1] > 10.0) {
        point.at[1] = 40.0;
    }
    app.scene.set_sketch(id, taller);
    assert!((volume(&mut app) - before - 2000.0).abs() < 1e-6);
}

#[test]
fn a_sketch_region_on_a_face_pushed_in_cuts_the_object_under_it() {
    let (mut app, a, _) = two_boxes("revolve-pocket", Vec3::new(400.0, 0.0, 0.0));
    app.reevaluate_for_test();
    let (lo, hi) = app.evaluated.node_world_bounds[&a];
    let plane = Xform::from_translation(Vec3::new(lo.x, lo.y, hi.z));
    let id = app.scene.add_sketch("Sketch", rectangle([1.0, 1.0], [3.0, 3.0]), &plane);
    let before = volume(&mut app);
    let view = View::new(app.scene.camera, app.viewport_rect);
    let cursor = screen(&view, Vec3::new(lo.x + 2.0, lo.y + 2.0, hi.z));
    let (target, _) = app.push_target_under(&view, cursor).expect("nothing under the pointer");
    let Ok(crate::push_pull_tool::Target::Region(hit, owner)) = target else {
        panic!("the face won over the sketch on it")
    };
    assert_eq!(hit.node, id);
    assert_eq!(owner, Some(a));
    crate::push_pull_tool::region_commit(&mut app, &hit, owner, -2.0);
    assert!((before - volume(&mut app) - 8.0).abs() < 1e-6, "the pocket did not cut 2 x 2 x 2 out");
}

#[test]
fn a_sketch_region_revolves_about_a_line_of_its_own_sketch() {
    // On the ground, from 10 to 20 off the line x = 10 ... with its own left edge as the axis.
    let plane = Xform::from_translation(Vec3::new(-200.0, 0.0, 0.0));
    let (mut app, _, view) = with_sketch("revolve-region", rectangle([10.0, 0.0], [20.0, 20.0]), plane);
    let before = volume(&mut app);
    let hit = app.region_under(&view, screen(&view, Vec3::new(-185.0, 10.0, 0.0))).unwrap();
    let profile = Target::Region(hit, None);
    let axis = match app.revolve_axis_under(&view, screen(&view, Vec3::new(-190.0, 10.0, 0.0)), &profile) {
        Some(Ok(axis)) => axis,
        _ => panic!("the sketch's left edge was not taken as the axis"),
    };
    assert!(matches!(axis.stored, simple3d_core::scene::RevolveAxis::Line(_)));
    app.revolve.set_profile(profile);
    app.revolve.set_axis(axis);
    app.revolve.angle = 90.0;
    commit::commit(&mut app);
    assert!(app.revolve.profile.is_none(), "the tool did not start over");
    let solid = app.primary().unwrap();
    assert!(app.scene.node(solid).feature().is_some_and(|f| f.is_revolve()));
    // A quarter of a solid cylinder of radius 10 and length 20, about the edge.
    let expected = std::f64::consts::PI * 100.0 * 20.0 / 4.0;
    let added = volume(&mut app) - before;
    assert!((added - expected).abs() / expected < 0.01, "added {added}, not {expected}");
}

#[test]
fn a_face_turned_into_its_own_object_about_an_edge_cuts_it() {
    let (mut app, a, _) = two_boxes("revolve-face", Vec3::new(400.0, 0.0, 0.0));
    app.reevaluate_for_test();
    let (lo, hi) = app.evaluated.node_world_bounds[&a];
    let before = volume(&mut app);
    let view = View::new(app.scene.camera, app.viewport_rect);
    let middle = Vec3::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0, hi.z);
    let (target, _) = app.push_target_under(&view, screen(&view, middle)).unwrap();
    let profile = target.ok().expect("the top face cannot be revolved");
    // The top face's edge along Y at the low X side.
    let edge = Vec3::new(lo.x, (lo.y + hi.y) / 2.0, hi.z);
    let axis = match app.revolve_axis_under(&view, screen(&view, edge), &profile) {
        Some(Ok(axis)) => axis,
        other => panic!("the edge was not taken as the axis: {:?}", other.map(|r| r.err())),
    };
    // Whichever way turns the face down into the box.
    app.revolve.set_profile(profile);
    app.revolve.set_axis(axis.clone());
    app.revolve.angle = 90.0;
    app.revolve.flip = angle::outward(&axis, 90.0);
    assert!(app.revolve.cuts(), "turned into the box, it does not cut");
    commit::commit(&mut app);
    let side = hi.x - lo.x;
    let expected = std::f64::consts::PI * side * side / 4.0 * (hi.y - lo.y);
    let cut = before - volume(&mut app);
    assert!((cut - expected).abs() / expected < 0.02, "cut {cut}, not {expected}");
}

#[test]
fn a_world_axis_parallel_to_an_offset_profile_is_taken_in_its_plane() {
    // On a plane 50 up, from y = 10 to 20: the world X axis runs parallel to it, 50 below.
    let plane = Xform::from_translation(Vec3::new(-200.0, 0.0, 50.0));
    let (mut app, _, view) = with_sketch("revolve-world", rectangle([0.0, 10.0], [20.0, 20.0]), plane);
    let before = volume(&mut app);
    let hit = app.region_under(&view, screen(&view, Vec3::new(-190.0, 15.0, 50.0))).unwrap();
    let profile = Target::Region(hit, None);
    assert!(axis::world(&profile, 2).is_err(), "the Z axis crosses the plane");
    let x = axis::world(&profile, 0).expect("the X axis runs parallel to the plane");
    assert!((x.world.0.z - 50.0).abs() < 1e-9 && x.world.0.y.abs() < 1e-9, "not over the axis in the plane");
    app.revolve.set_profile(profile);
    app.revolve.set_axis(x);
    // The axis survives picking the profile again.
    let again = app.revolve.profile.clone().unwrap();
    app.revolve.set_profile(again);
    assert!(app.revolve.axis.is_some(), "the world axis was dropped with a new profile");
    app.revolve.angle = 360.0;
    commit::commit(&mut app);
    // A tube from radius 10 to 20, 20 long.
    let expected = std::f64::consts::PI * (400.0 - 100.0) * 20.0;
    let added = volume(&mut app) - before;
    assert!((added - expected).abs() / expected < 0.01, "added {added}, not {expected}");
}
