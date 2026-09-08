//! The portable form a subtree is written to and read back from.

use super::*;
use crate::primitive::Params;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn subtree_round_trips_through_the_portable_form() {
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Intersection, root, 0);
    let child = box_at(&mut scene, group, 7.0);
    scene.get_mut(child).unwrap().rotation = Vec3::new(0.0, 45.0, 0.0);
    scene.get_mut(child).unwrap().name = "Special".into();

    let data = scene.export_subtree(group).unwrap();
    let json = serde_json::to_string(&data).unwrap();
    let back: NodeData = serde_json::from_str(&json).unwrap();
    let pasted = scene.import_subtree(&back, root, 1).unwrap();
    let pasted_child = scene.node(pasted).children[0];
    assert_eq!(scene.node(pasted).group_op(), Some(GroupOp::Intersection));
    assert_eq!(scene.node(pasted_child).name, "Special");
    assert_eq!(scene.node(pasted_child).rotation, Vec3::new(0.0, 45.0, 0.0));
    assert_eq!(scene.node(pasted_child).position, Vec3::new(7.0, 0.0, 0.0));
}

#[test]
pub(crate) fn a_pattern_holds_children_and_round_trips_through_the_portable_form() {
    use crate::primitive::ParamValue;
    let mut scene = Scene::new();
    let root = scene.root();
    let pat = scene.add_pattern(root, 0);
    assert!(scene.node(pat).is_pattern());
    assert!(scene.node(pat).can_hold_children(), "a pattern must be able to hold children");
    // A child can be reparented into it, the way a group takes one.
    let child = box_at(&mut scene, root, 3.0);
    scene.reparent(child, pat, 0).unwrap();
    assert_eq!(scene.node(pat).children, vec![child]);
    scene.get_mut(pat).unwrap().params_mut().unwrap().insert("count".into(), ParamValue::Count(5));

    let data = scene.export_subtree(pat).unwrap();
    assert_eq!(data.type_id, "pattern");
    let json = serde_json::to_string(&data).unwrap();
    let back: NodeData = serde_json::from_str(&json).unwrap();
    let copy = scene.import_subtree(&back, root, 1).unwrap();
    assert!(scene.node(copy).is_pattern());
    assert_eq!(scene.node(copy).params().unwrap().get("count"), Some(&ParamValue::Count(5)));
    assert_eq!(scene.node(copy).children.len(), 1, "the repeated child was lost");
}

#[test]
pub(crate) fn importing_an_unknown_primitive_type_leaves_no_partial_subtree() {
    let mut scene = Scene::new();
    let root = scene.root();
    let before = scene.len();
    let data = NodeData {
        name: "Group".into(),
        type_id: "group".into(),
        op: Some(GroupOp::Union),
        position: Vec3::ZERO,
        rotation: Vec3::ZERO,
        scale: Vec3::ONE,
        anchor: Anchor::Centre,
        visible: true,
        ghost: false,
        colour: None,
        segments: None,
        export_body: None,
        extracted: false,
        mesh: None,
        original: None,
        tiling: None,
        params: Params::new(),
        children: vec![NodeData {
            name: "From the future".into(),
            type_id: "hyperboloid".into(),
            op: None,
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
            anchor: Anchor::Centre,
            visible: true,
            ghost: false,
            colour: None,
            segments: None,
            export_body: None,
            extracted: false,
            mesh: None,
            original: None,
            tiling: None,
            params: Params::new(),
            children: vec![],
        }],
    };
    assert!(scene.import_subtree(&data, root, 0).is_none());
    assert_eq!(scene.len(), before);
    assert!(scene.node(root).children.is_empty());
}

#[test]
pub(crate) fn a_node_has_three_visibility_states_and_they_survive_the_file() {
    // Issue 21: hidden used to mean "invisible, unless the document-wide
    // ghost switch is on, in which case it means translucent" -- so the only
    // states the interface could reach were visible and ghost, and there was
    // no way to say "this one is gone" while another was being positioned.
    let mut scene = Scene::new();
    let root = scene.root();
    let a = scene.add_primitive("plate", root, 0).unwrap();
    assert_eq!(scene.node(a).visibility(), Visibility::Visible);

    for state in Visibility::ALL {
        scene.get_mut(a).unwrap().set_visibility(state);
        assert_eq!(scene.node(a).visibility(), state);
        // Anything but visible is out of the model, ghost included: a ghost
        // is drawn, never built.
        assert_eq!(scene.node(a).visible, state == Visibility::Visible);

        let data = scene.export_subtree(a).unwrap();
        let mut other = Scene::new();
        let other_root = other.root();
        let copy = other.import_subtree(&data, other_root, 0).unwrap();
        assert_eq!(other.node(copy).visibility(), state, "{state:?} did not survive the round trip");
    }
}
