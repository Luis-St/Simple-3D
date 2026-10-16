//! Duplicating with the last move repeated (issue 121).

use super::*;
use simple3d_core::keymap::Command;
use simple3d_geom::Vec3;

/// Duplicate `a` with Ctrl+D and give the copy a move and a turn.
fn moved_copy(app: &mut App, a: simple3d_core::scene::NodeId) -> simple3d_core::scene::NodeId {
    app.select_only(a);
    app.run(Command::Duplicate);
    let first = app.primary().unwrap();
    let node = app.scene.get_mut(first).unwrap();
    node.position = Vec3::new(15.0, 0.0, 0.0);
    node.rotation = Vec3::new(0.0, 0.0, 30.0);
    first
}

#[test]
fn repeating_puts_the_next_copy_as_far_on_again_and_keeps_going() {
    let (mut app, a, _) = two_boxes("repeat-duplicate", Vec3::new(0.0, 80.0, 0.0));
    moved_copy(&mut app, a);
    app.run(Command::RepeatDuplicate);
    let node = app.scene.node(app.primary().unwrap());
    // The step turns 30 degrees about the copy's own origin and moves 15 mm, so it goes on turning.
    let want = simple3d_core::xform::Xform::from_pos_rot(Vec3::new(15.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 30.0))
        .point(Vec3::new(15.0, 0.0, 0.0));
    assert!((node.position - want).length() < 1e-9, "{:?} vs {want:?}", node.position);
    assert!((node.rotation.z - 60.0).abs() < 1e-9, "{:?}", node.rotation);
    app.run(Command::RepeatDuplicate);
    assert!((app.scene.node(app.primary().unwrap()).rotation.z - 90.0).abs() < 1e-9);
}

#[test]
fn plain_duplicate_of_a_moved_copy_stays_in_place() {
    let (mut app, a, _) = two_boxes("plain-duplicate", Vec3::new(0.0, 80.0, 0.0));
    moved_copy(&mut app, a);
    app.run(Command::Duplicate);
    assert_eq!(app.scene.node(app.primary().unwrap()).position, Vec3::new(15.0, 0.0, 0.0));
}

#[test]
fn held_it_previews_and_let_go_it_copies() {
    let (mut app, a, _) = two_boxes("repeat-hold", Vec3::new(0.0, 80.0, 0.0));
    let first = moved_copy(&mut app, a);
    app.reevaluate_for_test();
    let count = app.scene.depth_first().len();
    app.start_repeat_hold();
    app.follow_repeat_hold(true);
    assert_eq!(app.scene.depth_first().len(), count, "a copy was made while still held");
    let templates = app.repeat_templates();
    assert_eq!(templates.len(), 1, "no preview while held");
    assert_eq!(templates[0].0, first);
    // The preview carries the copy's body exactly where the copy lands.
    let at = app.scene.node(first).position;
    let landed = templates[0].1.point(at);
    app.follow_repeat_hold(false);
    assert_eq!(app.scene.depth_first().len(), count + 1, "letting go made no copy");
    assert!((app.scene.node(app.primary().unwrap()).position - landed).length() < 1e-9);
    assert!(app.repeat_templates().is_empty(), "the preview outlived the hold");
}
