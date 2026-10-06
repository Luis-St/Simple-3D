//! Taking a rounding the scene holds out as nodes of its own (issue 88).

use super::push_pull::{evaluate, volume};
use super::round_edits::{an_l, chamfer};
use crate::scene::{Body, GroupOp, ObjectEdit, RoundEdit, RoundKind};

#[test]
pub(crate) fn a_rounding_the_scene_holds_extracts_as_a_cut_around_each_object_it_reaches() {
    let (mut scene, boxes, edges, corner) = an_l();
    let root = scene.root();
    let out = evaluate(&scene);
    let world = RoundEdit { kind: RoundKind::Round, segments: 8, joints: vec![corner], ..chamfer(2.0) };
    for (holder, edit) in scene.round_edits(&edges, &[], &world, &out.node_frames) {
        assert_eq!(holder, root);
        scene.push_edit(holder, ObjectEdit::Round(edit));
    }
    let rounded = evaluate(&scene);
    // Remade against the model as it was without the rounding, as the properties panel does.
    let made = scene
        .extract_round_edit(root, 0, &rounded.node_frames, &rounded.node_world_bounds, &out.mesh)
        .expect("the scene's rounding was not taken out");
    assert!(scene.node(root).edits.is_empty(), "the rounding stayed on the scene");
    // Each box the cutter reaches is in a difference of its own with a copy of it.
    assert_eq!(made.len(), boxes.len());
    for (&cutter, &block) in made.iter().zip(&boxes) {
        assert!(matches!(scene.node(cutter).body, Body::Mesh { .. }));
        let cut = scene.node(cutter).parent.unwrap();
        assert_eq!(scene.node(cut).group_op(), Some(GroupOp::Difference));
        assert_eq!(scene.node(cut).parent, Some(root));
        assert_eq!(scene.node(cut).children, vec![block, cutter]);
    }
    let after = evaluate(&scene);
    assert!(after.errors.is_empty(), "{:?}", after.errors);
    assert!(
        (volume(&after) - volume(&rounded)).abs() < 1e-6,
        "the model changed: {} to {}",
        volume(&rounded),
        volume(&after)
    );
}
