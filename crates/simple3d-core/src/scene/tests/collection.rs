//! Collections and the pieces they hold.

use super::*;

/// A collection holding `pieces` nameless mesh pieces, standing where a
/// two-box difference stood: the shape both halves of issue 82 leave behind.
pub(crate) fn collection_of(scene: &mut Scene, pieces: usize) -> (NodeId, Vec<NodeId>) {
    let root = scene.root();
    let group = scene.add_group(GroupOp::Difference, root, 0);
    box_at(scene, group, 0.0);
    box_at(scene, group, 5.0);
    let original = scene.export_subtree(group).unwrap();
    scene.remove(group);
    let split = scene.add_split(original, None, root, 0);
    let made = (0..pieces)
        .map(|i| {
            let mesh = crate::mesh_data::MeshData::new(simple3d_geom::primitives::box_mesh(1.0, 1.0, 1.0));
            scene.add_mesh(&format!("Piece {i}"), mesh, split, i)
        })
        .collect();
    (split, made)
}

#[test]
pub(crate) fn a_collection_keeps_its_pieces_out_of_the_tree() {
    // The point of issue 82's rework: the pieces are real nodes -- they
    // evaluate, export and take a transform apiece -- but the tree shows the
    // collection as one object however many thousand it is in.
    let mut scene = Scene::new();
    let (split, pieces) = collection_of(&mut scene, 4);
    assert!(scene.is_collection(split));
    assert!(scene.has_row(split), "the collection itself must be a row");
    assert!(scene.row_children(split).is_empty(), "the pieces were drawn in the tree");
    for &piece in &pieces {
        assert!(!scene.has_row(piece), "a piece inside the collection has a row");
        assert_eq!(scene.row_for(piece), split, "a click on a piece must mean the collection");
    }
    // And the pieces are still there, whatever the tree draws.
    assert_eq!(scene.node(split).children.len(), 4);
}

#[test]
pub(crate) fn extracting_a_piece_gives_it_a_row_and_putting_it_back_takes_it_away() {
    let mut scene = Scene::new();
    let (split, pieces) = collection_of(&mut scene, 4);

    assert_eq!(scene.extract_pieces(split, &pieces[..2]), 2);
    assert_eq!(scene.row_children(split), pieces[..2].to_vec(), "the extracted pieces are the rows");
    assert!(scene.has_row(pieces[0]));
    assert_eq!(scene.row_for(pieces[0]), pieces[0], "an extracted piece answers as itself");
    assert_eq!(scene.row_for(pieces[2]), split, "an untouched piece still answers as the collection");
    // Extracting the same piece twice is not two extractions.
    assert_eq!(scene.extract_pieces(split, &pieces[..2]), 0);
    assert!(scene.is_collection(split), "extracting some pieces dissolved the collection");

    assert_eq!(scene.return_pieces(split, &pieces[..1]), 1);
    assert_eq!(scene.row_children(split), vec![pieces[1]]);
}

#[test]
pub(crate) fn a_piece_that_is_not_this_collections_is_not_extracted_from_it() {
    // The list a panel hands over is what is ticked, and what is ticked can
    // outlive the collection it was ticked in.
    let mut scene = Scene::new();
    let (split, pieces) = collection_of(&mut scene, 2);
    let root = scene.root();
    let stranger = box_at(&mut scene, root, 20.0);
    assert_eq!(scene.extract_pieces(split, &[stranger, pieces[0]]), 1);
    assert!(!scene.node(stranger).extracted, "a node from elsewhere was marked");
}

#[test]
pub(crate) fn emptying_a_collection_leaves_an_ordinary_union_group() {
    // What extracting the last piece comes to: with nothing left inside it a
    // collection is a container holding a list of objects, which is a union
    // group -- and the recipe goes with it, which is why the application
    // asks first.
    let mut scene = Scene::new();
    let (split, pieces) = collection_of(&mut scene, 3);
    assert!(scene.dissolve_collection(split));
    assert!(!scene.is_collection(split));
    assert_eq!(scene.node(split).group_op(), Some(GroupOp::Union));
    assert!(scene.node(split).split_original().is_none(), "the recipe outlived the collection");
    assert_eq!(scene.row_children(split), pieces, "the pieces did not become ordinary rows");
    for &piece in &pieces {
        assert!(scene.has_row(piece));
        assert!(!scene.node(piece).extracted, "the mark outlived the collection it meant something in");
    }
    assert!(!scene.dissolve_collection(split), "a union group is not a collection to dissolve");
}

#[test]
pub(crate) fn something_dropped_into_a_collection_arrives_with_a_row() {
    // A collection hides its *pieces*. Something the user carried in by hand
    // is not one of them, and vanishing on release is not a move anybody
    // aimed for.
    let mut scene = Scene::new();
    let (split, _) = collection_of(&mut scene, 2);
    let root = scene.root();
    let boxed = box_at(&mut scene, root, 20.0);
    scene.reparent(boxed, split, 0).expect("a collection can hold children");
    assert!(scene.node(boxed).extracted);
    assert!(scene.has_row(boxed), "a shape dropped into a collection disappeared");

    // And dragged out again it loses a mark that means nothing there.
    scene.reparent(boxed, root, 0).expect("it can come out again");
    assert!(!scene.node(boxed).extracted);
    assert!(scene.has_row(boxed));
}

#[test]
pub(crate) fn which_pieces_are_extracted_survives_the_portable_form() {
    let mut scene = Scene::new();
    let (split, pieces) = collection_of(&mut scene, 3);
    scene.extract_pieces(split, &pieces[1..2]);

    let data = scene.export_subtree(split).unwrap();
    let json = serde_json::to_string(&data).unwrap();
    // The mark is absent from every node that does not carry one, so a
    // project that has never split anything writes exactly what it used to.
    assert_eq!(json.matches("\"extracted\"").count(), 1, "{json}");
    let back: NodeData = serde_json::from_str(&json).unwrap();

    let mut other = Scene::new();
    let other_root = other.root();
    let copy = other.import_subtree(&back, other_root, 0).expect("a collection imports");
    assert_eq!(other.row_children(copy).len(), 1, "the extracted piece lost its row over the round trip");
    assert_eq!(other.node(other.row_children(copy)[0]).name, "Piece 1");
}

#[test]
pub(crate) fn nothing_is_added_inside_a_collection_by_accident() {
    // The tree does not open a collection, so an Add with one selected must
    // not put a shape somewhere it cannot be seen. Dragging one in still
    // works: that is aimed at rather than defaulted to.
    let mut scene = Scene::new();
    let (split, _) = collection_of(&mut scene, 2);
    let root = scene.root();
    assert_eq!(scene.insertion_point(Some(split)), (root, 1), "an Add landed inside the collection");
}
