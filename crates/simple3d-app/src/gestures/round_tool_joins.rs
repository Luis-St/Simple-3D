//! The round tool's corners between edges of different objects, and edges merged across them
//! (issue 88).

use super::round_tool::click;
use super::*;
use simple3d_core::keymap::Command;
use simple3d_core::scene::ObjectEdit;
use simple3d_geom::Vec3;

/// Three boxes of an L, the front right quarter left out, seen looking into the notch. Returns the
/// height of their tops.
fn an_l(harness: &mut Harness<'_, App>) -> f64 {
    let app = harness.state_mut();
    let root = app.scene.root();
    for id in app.scene.node(root).children.clone() {
        app.scene.remove(id);
    }
    for (i, (x, y)) in [(-10.0, -10.0), (-10.0, 10.0), (10.0, 10.0)].into_iter().enumerate() {
        let id = app.scene.add_primitive("box", root, i).unwrap();
        app.scene.get_mut(id).unwrap().position = Vec3::new(x, y, 0.0);
    }
    app.reevaluate_for_test();
    let top = app.evaluated.bounds.unwrap().1.z;
    app.scene.camera.yaw = -45.0;
    app.scene.camera.pitch = 35.0;
    app.scene.camera.distance = 80.0;
    app.scene.camera.target = Vec3::new(0.0, 0.0, top - 5.0);
    top
}

#[test]
pub(crate) fn an_inside_corner_is_left_alone_until_it_is_picked() {
    let mut harness = harness("round-tool-joint");
    let top = an_l(&mut harness);
    harness.state_mut().run(Command::RoundEdges);
    harness.step();
    assert!(!harness.state().round_tool.as_ref().unwrap().blend_corners, "corners are treated unpicked");

    let view = harness.state().current_view();
    let on_screen = |p: Vec3| view.project(p).unwrap().0;
    let corner = Vec3::new(0.0, 0.0, top);
    for p in [Vec3::new(8.0, 0.0, top), Vec3::new(0.0, -8.0, top)] {
        click(&mut harness, on_screen(p));
    }
    let app = harness.state();
    assert_eq!(app.round_tool.as_ref().unwrap().edges.len(), 2, "the notch's edges were not picked");
    let holders: Vec<_> = app.round_tool.as_ref().unwrap().draft.iter().map(|(holder, _)| *holder).collect();
    assert_eq!(holders.len(), 2, "unpicked, the corner joined the two boxes' roundings");

    assert!(matches!(app.round_pick_at(&view, on_screen(corner)), Some(crate::round_tool::pick::Pick::Joint(_))));
    click(&mut harness, on_screen(corner));
    let app = harness.state();
    let tool = app.round_tool.as_ref().unwrap();
    assert_eq!(tool.joints.len(), 1);
    match &tool.draft[..] {
        [(holder, ObjectEdit::Round(edit))] => {
            assert_eq!(*holder, app.scene.root(), "the picked corner did not join the roundings on the scene");
            assert_eq!(edit.joints.len(), 1);
        }
        draft => panic!("{} edits in the draft", draft.len()),
    }
}

#[test]
pub(crate) fn the_inside_corner_between_earlier_roundings_can_be_picked_later() {
    let mut harness = harness("round-tool-late-joint");
    let top = an_l(&mut harness);
    let corner = Vec3::new(0.0, 0.0, top);
    let on_screen = |harness: &Harness<'_, App>, p: Vec3| harness.state().current_view().project(p).unwrap().0;
    harness.state_mut().run(Command::RoundEdges);
    harness.step();
    for p in [Vec3::new(8.0, 0.0, top), Vec3::new(0.0, -8.0, top)] {
        let at = on_screen(&harness, p);
        click(&mut harness, at);
    }
    harness.state_mut().apply_round_tool();
    harness.state_mut().reevaluate_for_test();
    let rounded = |app: &App| {
        let root = app.scene.root();
        let mut ids = app.scene.node(root).children.clone();
        ids.push(root);
        ids.into_iter().filter(|&id| !app.scene.node(id).edits.is_empty()).collect::<Vec<_>>()
    };
    assert_eq!(rounded(harness.state()).len(), 2, "each box holds its own rounding");

    harness.state_mut().run(Command::RoundEdges);
    harness.step();
    let view = harness.state().current_view();
    let at = on_screen(&harness, corner);
    assert!(matches!(harness.state().round_pick_at(&view, at), Some(crate::round_tool::pick::Pick::Joint(_))));
    click(&mut harness, at);
    let app = harness.state();
    let root = app.scene.root();
    assert_eq!(rounded(app), vec![root], "the draft did not join the roundings on the scene");
    match &app.scene.node(root).edits[..] {
        [ObjectEdit::Round(edit)] => assert_eq!((edit.edges.len(), edit.joints.len()), (2, 1)),
        edits => panic!("{} edits on the scene", edits.len()),
    }

    harness.state_mut().cancel_round_tool();
    assert_eq!(rounded(harness.state()).len(), 2, "cancelling did not give the boxes their roundings back");
    harness.state_mut().run(Command::RoundEdges);
    harness.step();
    click(&mut harness, at);
    harness.state_mut().apply_round_tool();
    assert_eq!(rounded(harness.state()), vec![root]);
    harness.state_mut().run(Command::Undo);
    assert_eq!(rounded(harness.state()).len(), 2, "the step did not undo the join");
}

#[test]
pub(crate) fn an_edge_running_over_two_boxes_is_one_edge_on_both_unless_merging_is_off() {
    let mut harness = harness("round-tool-merge");
    let app = harness.state_mut();
    let root = app.scene.root();
    for id in app.scene.node(root).children.clone() {
        app.scene.remove(id);
    }
    let boxes: Vec<_> = [-10.0, 10.0]
        .into_iter()
        .enumerate()
        .map(|(i, x)| {
            let id = app.scene.add_primitive("box", root, i).unwrap();
            app.scene.get_mut(id).unwrap().position = Vec3::new(x, 0.0, 0.0);
            id
        })
        .collect();
    app.reevaluate_for_test();
    let (lo, hi) = app.evaluated.bounds.unwrap();
    app.scene.camera.yaw = -30.0;
    app.scene.camera.pitch = 30.0;
    app.scene.camera.distance = 100.0;
    app.scene.camera.target = Vec3::new(0.0, 0.0, hi.z - 5.0);
    harness.state_mut().run(Command::RoundEdges);
    harness.step();
    let at = harness.state().current_view().project(Vec3::new(5.0, lo.y, hi.z)).unwrap().0;

    click(&mut harness, at);
    let tool = harness.state().round_tool.as_ref().unwrap();
    assert!(tool.merge_edges, "merging is on to begin with");
    assert!((tool.edges[0].length() - 40.0).abs() < 1e-6, "the edge did not run over both boxes");
    match &tool.draft[..] {
        [(holder, ObjectEdit::Round(edit))] => {
            assert_eq!(*holder, root, "the merged edge was held by one of its boxes");
            assert_eq!(edit.edges.len(), 1);
        }
        draft => panic!("{} edits in the draft", draft.len()),
    }

    harness.state_mut().round_tool.as_mut().unwrap().merge_edges = false;
    harness.step();
    assert!(harness.state().round_tool.as_ref().unwrap().edges.is_empty(), "the merged pick outlived merging");
    click(&mut harness, at);
    let tool = harness.state().round_tool.as_ref().unwrap();
    assert!((tool.edges[0].length() - 20.0).abs() < 1e-6, "unmerged, the edge still ran over both boxes");
    let holders: Vec<_> = tool.draft.iter().map(|(holder, _)| *holder).collect();
    assert_eq!(holders, vec![boxes[1]]);
}

/// The L's volume after bevelling the two top edges meeting at its back right corner: one running over
/// two boxes, held by the scene, and one of a single box; in one step or one after the other.
fn bevel_back_corner(steps: bool, extend: bool) -> f64 {
    let mut harness = harness("round-tool-extend");
    let top = an_l(&mut harness);
    harness.state_mut().scene.camera.yaw = 135.0;
    harness.step();
    let corner = Vec3::new(20.0, 20.0, top);
    let on_screen = |harness: &Harness<'_, App>, p: Vec3| harness.state().current_view().project(p).unwrap().0;
    let open = |harness: &mut Harness<'_, App>| {
        harness.state_mut().run(Command::RoundEdges);
        harness.step();
        let tool = harness.state_mut().round_tool.as_mut().unwrap();
        tool.kind = crate::round_tool::Kind::Chamfer;
        tool.extend_edges = extend;
    };
    open(&mut harness);
    let at = on_screen(&harness, corner + Vec3::new(-6.0, 0.0, 0.0));
    click(&mut harness, at);
    if steps {
        harness.state_mut().apply_round_tool();
        harness.state_mut().reevaluate_for_test();
        open(&mut harness);
        harness.step();
    }
    let at = on_screen(&harness, corner + Vec3::new(0.0, -6.0, 0.0));
    click(&mut harness, at);
    harness.state_mut().apply_round_tool();
    harness.state_mut().reevaluate_for_test();
    harness.state().evaluated.mesh.signed_volume()
}

#[test]
pub(crate) fn an_edge_extended_to_the_end_of_the_body_leaves_no_stub_at_a_corner_bevelled_earlier() {
    // The second edge, found on the bevelled model, stops short of the corner; unextended it left a
    // corner of 0.17 mm^3 (a 1 mm bevel's tetrahedron) standing there.
    let together = bevel_back_corner(false, false);
    assert!(bevel_back_corner(true, false) - together > 0.1, "no stub to begin with");
    assert!((bevel_back_corner(true, true) - together).abs() < 1e-6, "extended, a stub was left");
}
