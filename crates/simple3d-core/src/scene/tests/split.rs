//! A split node: what it stands for, and what it gives back.

use super::*;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn a_split_stands_where_the_shape_stood_and_gives_it_back_on_request() {
    // Issue 82: breaking a shape apart must be reversible, so the node the
    // pieces go under carries the shape itself, and putting it back is one
    // call rather than a rebuild by hand.
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Union, root, 0);
    scene.get_mut(group).unwrap().name = "Bracket".into();
    scene.get_mut(group).unwrap().position = Vec3::new(5.0, 0.0, 0.0);
    scene.get_mut(group).unwrap().anchor = Anchor::Base;
    box_at(&mut scene, group, 0.0);
    box_at(&mut scene, group, 60.0);

    let original = scene.export_subtree(group).unwrap();
    scene.remove(group);
    let split = scene.add_split(original, None, root, 0);
    assert!(scene.node(split).is_split());
    // The shape's own properties came across, name included.
    assert_eq!(scene.node(split).name, "Bracket");
    assert_eq!(scene.node(split).position, Vec3::new(5.0, 0.0, 0.0));
    assert_eq!(scene.node(split).anchor, Anchor::Base);
    assert_eq!(scene.node(split).group_op(), None, "a split is not a group");
    assert_eq!(scene.node(split).combine_op(), Some(GroupOp::Union), "its pieces stand side by side");
    assert!(scene.can_split_for_export(split), "its pieces are separate solids and an export can say so");

    // Whatever is done to the node afterwards is what the restored shape
    // wears: the recipe says what it is, the node says where it is.
    scene.get_mut(split).unwrap().position = Vec3::new(5.0, 0.0, 40.0);
    scene.get_mut(split).unwrap().name = "Bracket, in pieces".into();
    let back = scene.restore_split(split).unwrap();
    assert!(!scene.contains(split), "the split outlived the shape it gave back");
    assert_eq!(scene.node(back).group_op(), Some(GroupOp::Union));
    assert_eq!(scene.node(back).children.len(), 2, "the operands did not come back");
    assert_eq!(scene.node(back).name, "Bracket, in pieces");
    assert_eq!(scene.node(back).position, Vec3::new(5.0, 0.0, 40.0));
    assert_eq!(scene.node(back).anchor, Anchor::Base);
    assert_eq!(scene.node(root).children, vec![back], "it came back somewhere else in the tree");
}

#[test]
pub(crate) fn a_split_survives_the_portable_form_with_its_shape_and_its_pieces() {
    let mut scene = Scene::new();
    let root = scene.root();
    let group = scene.add_group(GroupOp::Difference, root, 0);
    box_at(&mut scene, group, 0.0);
    box_at(&mut scene, group, 5.0);
    let original = scene.export_subtree(group).unwrap();
    scene.remove(group);
    let split = scene.add_split(original, None, root, 0);
    let piece = crate::mesh_data::MeshData::new(simple3d_geom::primitives::box_mesh(10.0, 10.0, 10.0));
    scene.add_mesh("Piece", piece, split, 0);

    let data = scene.export_subtree(split).unwrap();
    assert_eq!(data.type_id, "split");
    let mut other = Scene::new();
    let other_root = other.root();
    let copy = other.import_subtree(&data, other_root, 0).expect("a split imports");
    assert!(other.node(copy).is_split());
    assert_eq!(other.node(copy).children.len(), 1, "the piece was lost");
    let back = other.restore_split(copy).expect("the shape rebuilds");
    assert_eq!(other.node(back).group_op(), Some(GroupOp::Difference));
    assert_eq!(other.node(back).children.len(), 2);

    // A split with no shape behind it is not a split, and is refused the way
    // a mesh with no geometry is rather than loaded as something else.
    let mut hollow = data.clone();
    hollow.original = None;
    assert!(other.import_subtree(&hollow, other_root, 0).is_none());
}
