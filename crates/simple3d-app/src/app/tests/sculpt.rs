//! The brush's working copy (issue 127): strokes edit a copy, and the document only sees the result.

use super::simplify::app_with_a_mesh;
use super::*;
use simple3d_core::keymap::Command;
use simple3d_core::scene::NodeId;
use simple3d_geom::Vec3;
use std::sync::Arc;

fn positions(app: &App, id: NodeId) -> Vec<Vec3> {
    app.scene.node(id).mesh().expect("it is a mesh").mesh.positions.clone()
}

/// A dab pushing a few vertices out by a millimetre along their normals.
fn dab(app: &mut App, vertices: std::ops::Range<u32>) {
    let sculpt = app.sculpt.as_mut().expect("a stroke runs");
    let moves: Vec<(u32, Vec3)> =
        vertices.map(|v| (v, sculpt.renderable.mesh.positions[v as usize] + sculpt.normals[v as usize])).collect();
    sculpt.move_vertices(&moves);
}

#[test]
fn a_stroke_in_progress_is_in_no_undo_step_and_no_save() {
    let (mut app, id) = app_with_a_mesh();
    let before = positions(&app, id);
    let original = app.scene.node(id).mesh().expect("a mesh").to_blob().positions;
    assert!(app.begin_stroke(id));
    dab(&mut app, 0..20);
    dab(&mut app, 10..30);
    assert_eq!(positions(&app, id), before, "the document holds the stroke before it ended");
    app.edit("Something else", None);
    let path = temp_config_dir("sculpt-save").join("stroke.s3d");
    app.save_to(&path);
    let saved = std::fs::read_to_string(&path).expect("the project was written");
    assert!(saved.contains(&original), "the save does not hold the mesh as it is in the document");
    app.cancel_stroke();
    app.run(Command::Undo);
    assert_eq!(positions(&app, id), before, "the undo step held the stroke");
}

#[test]
fn a_stroke_is_one_undo_step_back_to_the_mesh_before_it() {
    let (mut app, id) = app_with_a_mesh();
    let before = positions(&app, id);
    assert!(app.begin_stroke(id));
    for at in 0..5 {
        dab(&mut app, at * 4..at * 4 + 12);
    }
    app.end_stroke("Brush stroke");
    let after = positions(&app, id);
    assert_ne!(after, before);
    assert_eq!(app.history.undo_len(), 1, "a stroke of five dabs took other than one undo step");
    // Moved a millimetre along the normal, so the sphere grew there; the rest stayed exactly put.
    assert!((after[0].length() - before[0].length() - 1.0).abs() < 1e-9);
    assert_eq!(after[100..], before[100..]);
    app.run(Command::Undo);
    assert_eq!(positions(&app, id), before);
}

#[test]
fn cancelling_a_stroke_puts_the_copy_back_and_leaves_no_step() {
    let (mut app, id) = app_with_a_mesh();
    assert!(app.begin_stroke(id));
    let copy = app.sculpt.as_ref().expect("a session").renderable.mesh.positions.clone();
    dab(&mut app, 0..40);
    app.cancel_stroke();
    let sculpt = app.sculpt.as_ref().expect("the session stays");
    assert!(sculpt.stroke.is_none());
    assert_eq!(sculpt.renderable.mesh.positions, copy, "the copy kept the cancelled stroke");
    // Ending after a cancel has nothing to write.
    app.end_stroke("Brush stroke");
    assert_eq!(app.history.undo_len(), 0);
}

#[test]
fn the_next_stroke_carries_on_with_the_same_session() {
    let (mut app, id) = app_with_a_mesh();
    assert!(app.begin_stroke(id));
    dab(&mut app, 0..10);
    app.end_stroke("Brush stroke");
    let first = app.sculpt.as_ref().expect("a session").renderable.id;
    app.reevaluate_for_test();
    assert!(app.begin_stroke(id));
    let sculpt = app.sculpt.as_ref().expect("a session");
    assert!(Arc::ptr_eq(&sculpt.base, app.scene.node(id).mesh().expect("a mesh")), "the session was rebuilt");
    assert_ne!(sculpt.renderable.id, first, "a new stroke must go to the card whole once");
    // An undo puts another mesh on the node, and the session follows it.
    app.cancel_stroke();
    app.run(Command::Undo);
    app.reevaluate_for_test();
    assert!(app.begin_stroke(id));
    assert!(Arc::ptr_eq(&app.sculpt.as_ref().expect("a session").base, app.scene.node(id).mesh().expect("a mesh")));
}

#[test]
fn a_placed_and_scaled_mesh_is_written_back_in_its_own_coordinates() {
    let (mut app, id) = app_with_a_mesh();
    let node = app.scene.get_mut(id).expect("the node");
    node.position = Vec3::new(30.0, -10.0, 5.0);
    node.rotation = Vec3::new(0.0, 0.0, 40.0);
    node.scale = Vec3::new(2.0, 1.0, 1.0);
    app.reevaluate_for_test();
    let before = positions(&app, id);
    assert!(app.begin_stroke(id));
    let world = app.evaluated.node_meshes[&id].positions.clone();
    let sculpt = app.sculpt.as_ref().expect("a session");
    let close = |a: Vec3, b: Vec3| (a - b).length() < 1e-9;
    assert!(world.iter().zip(&sculpt.renderable.mesh.positions).all(|(&a, &b)| close(a, b)), "not placed as evaluated");
    let target = world[0] + Vec3::new(0.0, 0.0, 3.0);
    app.sculpt.as_mut().expect("a session").move_vertices(&[(0, target)]);
    app.end_stroke("Brush stroke");
    app.reevaluate_for_test();
    assert!(close(app.evaluated.node_meshes[&id].positions[0], target), "the vertex did not land where it was put");
    assert_eq!(positions(&app, id)[1..], before[1..]);
}
