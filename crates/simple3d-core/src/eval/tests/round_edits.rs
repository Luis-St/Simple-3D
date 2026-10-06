//! The round tool's edits kept on their object (issue 88): sized, carried along, dropped, taken out
//! and saved.

use super::push_pull::{evaluate, volume};
use super::*;
use crate::scene::{Body, GroupOp, ObjectEdit, RoundEdit, RoundKind, Scene};
use simple3d_geom::rounding::{feature_edges, Corner, FeatureEdge};
use std::f64::consts::PI;

/// A box and the world edge of its top running along X at its front, with its volume and length.
pub(super) fn a_box(scene: &mut Scene) -> (NodeId, FeatureEdge, f64) {
    let root = scene.root();
    let block = scene.add_primitive("box", root, 0).unwrap();
    let out = evaluate(scene);
    let (lo, hi) = out.bounds.unwrap();
    let edge = *feature_edges(&out.mesh)
        .iter()
        .find(|e| {
            let m = (e.a + e.b) / 2.0;
            (m.y - lo.y).abs() < 1e-9 && (m.z - hi.z).abs() < 1e-9
        })
        .expect("no top front edge");
    (block, edge, volume(&out))
}

pub(super) fn chamfer(size: f64) -> RoundEdit {
    RoundEdit {
        kind: RoundKind::Chamfer,
        size,
        segments: 8,
        edges: Vec::new(),
        corners: Vec::new(),
        joints: Vec::new(),
    }
}

/// Treat `edges` of `block` as `world` does, keeping the edits on their holders.
pub(super) fn treat(scene: &mut Scene, block: NodeId, edges: &[FeatureEdge], corners: &[Corner], world: &RoundEdit) {
    let out = evaluate(scene);
    let edges: Vec<_> = edges.iter().map(|&e| (block, e)).collect();
    let corners: Vec<_> = corners.iter().map(|c| (block, c.clone())).collect();
    for (holder, edit) in scene.round_edits(&edges, &corners, world, &out.node_frames) {
        assert!(scene.push_edit(holder, ObjectEdit::Round(edit)));
    }
}

#[test]
pub(crate) fn a_bevelled_edge_is_kept_on_its_box_and_resized_there() {
    let mut scene = Scene::new();
    let (block, edge, before) = a_box(&mut scene);
    treat(&mut scene, block, &[edge], &[], &chamfer(2.0));
    assert_eq!(scene.node(block).edits.len(), 1);
    assert_eq!(scene.node(scene.root()).children, vec![block], "the bevel made a node of its own");
    let removed = before - volume(&evaluate(&scene));
    assert!((removed - 2.0 * edge.length()).abs() < 1e-6, "removed {removed}");

    assert!(scene.set_edit_size(block, 0, 4.0));
    let removed = before - volume(&evaluate(&scene));
    assert!((removed - 8.0 * edge.length()).abs() < 1e-6, "removed {removed} at 4");
    assert!(matches!(scene.remove_edit(block, 0), Some(ObjectEdit::Round(_))));
    assert!((volume(&evaluate(&scene)) - before).abs() < 1e-9);
}

#[test]
pub(crate) fn a_rounded_edge_moves_turns_and_scales_with_its_box() {
    let mut scene = Scene::new();
    let (block, edge, before) = a_box(&mut scene);
    let round = RoundEdit { kind: RoundKind::Round, segments: 32, ..chamfer(3.0) };
    treat(&mut scene, block, &[edge], &[], &round);
    let removed = before - volume(&evaluate(&scene));
    let expected = (9.0 - PI * 9.0 / 4.0) * edge.length();
    // A little more than the true arc, which the 32 flat pieces stand inside.
    assert!((removed - expected).abs() < 0.1, "removed {removed}, an arc takes {expected}");

    scene.get_mut(block).unwrap().position = Vec3::new(5.0, -3.0, 0.0);
    scene.get_mut(block).unwrap().rotation = Vec3::new(0.0, 0.0, 30.0);
    let moved = before - volume(&evaluate(&scene));
    assert!((moved - removed).abs() < 1e-6, "moving the box changed the rounding: {moved} vs {removed}");
    scene.get_mut(block).unwrap().scale = Vec3::new(2.0, 2.0, 2.0);
    let scaled = 8.0 * before - volume(&evaluate(&scene));
    assert!((scaled - 8.0 * removed).abs() < 1e-3, "the rounding did not scale with the box: {scaled}");
}

#[test]
pub(crate) fn a_box_bevelled_at_a_corner_takes_the_corner_off_and_extracts_as_nodes() {
    let mut scene = Scene::new();
    let (block, _, before) = a_box(&mut scene);
    let out = evaluate(&scene);
    let edges = feature_edges(&out.mesh);
    let corner = Corner::find(&edges)[0].clone();
    treat(&mut scene, block, &[], &[corner], &chamfer(3.0));
    let edited = volume(&evaluate(&scene));
    assert!((before - edited - 27.0 / 6.0).abs() < 1e-6, "removed {}", before - edited);

    let out = evaluate(&scene);
    let made = scene.extract_round_edit(block, 0, &out.node_frames, &out.node_world_bounds, &out.mesh).unwrap();
    assert!(scene.node(block).edits.is_empty());
    assert_eq!(made.len(), 1);
    let cutter = made[0];
    assert_eq!(scene.node(cutter).name, "Box bevel");
    assert!(matches!(scene.node(cutter).body, Body::Mesh { .. }));
    let group = scene.node(cutter).parent.unwrap();
    assert_eq!(scene.node(group).group_op(), Some(GroupOp::Difference));
    assert!((volume(&evaluate(&scene)) - edited).abs() < 1e-6, "extracting changed the model");
}

#[test]
pub(crate) fn round_edits_round_trip_through_a_project_file_beside_push_pull_edits() {
    let mut scene = Scene::new();
    let (block, edge, _) = a_box(&mut scene);
    treat(&mut scene, block, &[edge], &[], &chamfer(2.0));
    let text = crate::project::to_string(&scene);
    assert!(text.contains("\"kind\": \"chamfer\""), "{text}");
    let back = crate::project::from_str(&text).unwrap();
    assert_eq!(back.node(block).edits, scene.node(block).edits);
    assert!((volume(&evaluate(&back)) - volume(&evaluate(&scene))).abs() < 1e-9);
}

#[test]
pub(crate) fn an_edge_without_room_for_the_size_is_left_as_it_is() {
    let mut scene = Scene::new();
    let (block, edge, before) = a_box(&mut scene);
    // Further back than either face goes: it would cut through the far side of the box.
    let (lo, hi) = evaluate(&scene).bounds.unwrap();
    let too_far = (hi - lo).x.max((hi - lo).z) + 1.0;
    treat(&mut scene, block, &[edge], &[], &chamfer(too_far));
    assert!((volume(&evaluate(&scene)) - before).abs() < 1e-9, "the bevel cut through the box");
    // Brought back within the faces, the same edit bevels it.
    assert!(scene.set_edit_size(block, 0, 2.0));
    assert!((before - volume(&evaluate(&scene)) - 2.0 * edge.length()).abs() < 1e-6);
}

/// Three boxes of an L, the front right quarter left out, with the two top edges of its notch, each
/// with the box it is on, and the inside corner where they meet.
pub(super) fn an_l() -> (Scene, Vec<NodeId>, Vec<(NodeId, FeatureEdge)>, Vec3) {
    let mut scene = Scene::new();
    let root = scene.root();
    let boxes: Vec<NodeId> = [(-10.0, -10.0), (-10.0, 10.0), (10.0, 10.0)]
        .iter()
        .enumerate()
        .map(|(i, &(x, y))| {
            let id = scene.add_primitive("box", root, i).unwrap();
            scene.get_mut(id).unwrap().position = Vec3::new(x, y, 0.0);
            id
        })
        .collect();
    let out = evaluate(&scene);
    let corner = Vec3::new(0.0, 0.0, out.bounds.unwrap().1.z);
    let source = |e: &FeatureEdge| NodeId::from(e.sources[0].max(e.sources[1]));
    let edges: Vec<(NodeId, FeatureEdge)> = feature_edges(&out.mesh)
        .into_iter()
        .filter(|e| e.convex && ((e.a - corner).length() < 1e-9 || (e.b - corner).length() < 1e-9))
        .map(|e| (source(&e), e))
        .collect();
    (scene, boxes, edges, corner)
}

#[test]
pub(crate) fn the_inside_corner_between_earlier_roundings_joins_them_on_the_scene() {
    let (mut scene, _, edges, corner) = an_l();
    let root = scene.root();
    let out = evaluate(&scene);
    let world = RoundEdit { kind: RoundKind::Round, segments: 16, ..chamfer(3.0) };
    for (holder, edit) in scene.round_edits(&edges, &[], &world, &out.node_frames) {
        scene.push_edit(holder, ObjectEdit::Round(edit));
    }
    let apart = evaluate(&scene);
    let spike = corner + Vec3::new(-0.3, 0.3, -0.3);
    assert!(simple3d_geom::rounding::contains_point(&apart.mesh, spike), "the corner was cut unpicked");

    let joins = scene.round_joins(&apart.node_frames);
    let join = joins.iter().find(|j| (j.at - corner).length() < 1e-9).expect("the corner is not offered");
    assert_eq!(join.parts.len(), 2, "the corner does not join both boxes' roundings");
    let (holder, edit) = scene.joined_round_edit(&join.parts, &[join.at], &apart.node_frames).unwrap();
    assert_eq!(holder, root);
    assert_eq!((edit.edges.len(), edit.joints.len()), (2, 1));
    for &(id, index) in join.parts.iter().rev() {
        scene.remove_edit(id, index);
    }
    scene.push_edit(holder, ObjectEdit::Round(edit));
    let joined = evaluate(&scene);
    assert!(joined.mesh.manifold_issue().is_none());
    assert!(!simple3d_geom::rounding::contains_point(&joined.mesh, spike), "the corner still stands");
    assert!(scene.round_joins(&joined.node_frames).iter().all(|j| (j.at - corner).length() > 1e-9));
}

#[test]
pub(crate) fn the_inside_corner_of_three_boxes_rounds_on_the_scene_and_takes_the_diagonal_box_in() {
    // Three boxes of an L, the front right quarter left out. The notch's top edges belong to the front
    // left and back right boxes; the back left one, diagonal to the notch, has the corner standing
    // between their rounds.
    let (mut scene, boxes, edges, corner) = an_l();
    let root = scene.root();
    let out = evaluate(&scene);
    // Not picked, the corner leaves each box its own rounding, ending flat there.
    let world = RoundEdit { kind: RoundKind::Round, segments: 16, ..chamfer(3.0) };
    assert_eq!(scene.round_edits(&edges, &[], &world, &out.node_frames).len(), 2);
    let world = RoundEdit { joints: vec![corner], ..world };
    let edits = scene.round_edits(&edges, &[], &world, &out.node_frames);
    assert_eq!(edits.len(), 1, "the meeting edges were held apart");
    assert_eq!(edits[0].0, root);
    for (holder, edit) in edits {
        scene.push_edit(holder, ObjectEdit::Round(edit));
    }
    let after = evaluate(&scene);
    assert!(after.mesh.manifold_issue().is_none());
    // The diagonal box's corner, between the two rounds, is cut away with them.
    let behind = corner + Vec3::new(-0.3, 0.3, -0.3);
    assert!(simple3d_geom::rounding::contains_point(&out.node_meshes[&boxes[1]], behind));
    assert!(!simple3d_geom::rounding::contains_point(&after.mesh, behind), "the corner still stands");
}

#[test]
pub(crate) fn a_rounding_held_by_the_scene_leaves_each_box_its_own_colour_where_it_cuts() {
    let (mut scene, boxes, edges, corner) = an_l();
    let root = scene.root();
    let colours = [[200, 60, 50], [70, 160, 70], [200, 60, 50]];
    for (&id, rgb) in boxes.iter().zip(colours) {
        scene.get_mut(id).unwrap().colour = Some(crate::scene::Colour(rgb));
    }
    let out = evaluate(&scene);
    let world = RoundEdit { kind: RoundKind::Round, segments: 16, joints: vec![corner], ..chamfer(3.0) };
    for (holder, edit) in scene.round_edits(&edges, &[], &world, &out.node_frames) {
        assert_eq!(holder, root);
        scene.push_edit(holder, ObjectEdit::Round(edit));
    }
    let after = evaluate(&scene);
    // The diagonal box's corner is cut by the rounds of its neighbours' edges. Cut out of the scene's
    // union, each wall ran on from a red box into it in one face and was painted red all over.
    let green = crate::scene::colour_tag(Some(crate::scene::Colour(colours[1])));
    let mut seen = 0;
    for t in 0..after.mesh.triangle_count() {
        let corners = after.mesh.corners(after.mesh.indices[t]);
        let centre = (corners[0] + corners[1] + corners[2]) / 3.0;
        for p in corners.map(|p| p + (centre - p) * 0.05) {
            if p.x < -1e-3 && p.y > 1e-3 && p.z > corner.z - 3.5 && p.z < corner.z - 1e-3 {
                seen += 1;
                assert_eq!(after.mesh.tag(t), green, "a face reaching the green box's corner at {p:?} is another colour");
                assert_eq!(after.mesh.source(t), crate::eval::source_of(boxes[1]), "at {p:?}");
            }
        }
    }
    assert!(seen > 0, "no faces near the corner");
}

