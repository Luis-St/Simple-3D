//! The pieces a split left, and taking them out of the collection.

use super::*;
use simple3d_core::keymap::Command;
use simple3d_core::scene::GroupOp;

/// A piece is named after the shape it came out of and numbered in a series
/// of its own (issue 82).
///
/// They used to be "Plate 1" and up, which is the series the *objects* use:
/// eighty pieces took eighty numbers out of it, and the next plate the user
/// added came out as "Plate 81".
#[test]
pub(crate) fn pieces_are_named_apart_from_the_objects_they_came_from() {
    let mut app = headless_app();
    split_with(&mut app, simple3d_geom::tiling::Tiling { size: 10.0, ..Default::default() });
    let split = app.primary().unwrap();
    let names: Vec<String> = app.scene.node(split).children.iter().map(|&c| app.scene.node(c).name.clone()).collect();
    let base = app.scene.node(split).name.clone();
    assert_eq!(names.first().map(String::as_str), Some(format!("{base} Piece 1").as_str()));
    assert_eq!(names.last().map(String::as_str), Some(format!("{base} Piece {}", names.len()).as_str()));

    // And the next object of that kind is the second one, not the ninth.
    let root = app.scene.root();
    let another = app.scene.add_primitive("plate", root, 0).expect("the plate is in the registry");
    assert_eq!(app.scene.node(another).name, format!("{base} 2"), "the pieces ate the objects' numbering");
}

#[test]
pub(crate) fn a_split_is_one_row_in_the_outliner_however_many_pieces_it_holds() {
    // The whole of why a collection exists: a hexagon tiling over a plate is
    // thousands of pieces, and thousands of rows is a tree nobody can find
    // anything in (issue 82).
    let mut app = headless_app();
    split_with(&mut app, simple3d_geom::tiling::Tiling { size: 10.0, ..Default::default() });
    let split = app.primary().unwrap();
    assert_eq!(app.scene.node(split).children.len(), 8);

    let rows = crate::panel_outliner::visible_rows(&app);
    assert!(rows.contains(&split), "the collection itself has no row");
    for &piece in &app.scene.node(split).children {
        assert!(!rows.contains(&piece), "a piece was drawn in the tree");
    }
}

#[test]
pub(crate) fn ticked_pieces_are_extracted_into_rows_of_their_own() {
    let mut app = headless_app();
    split_with(&mut app, simple3d_geom::tiling::Tiling { size: 10.0, ..Default::default() });
    let split = app.primary().unwrap();
    let pieces = app.scene.node(split).children.clone();

    // Nothing ticked is a warning rather than an edit.
    let before = app.history.undo_len();
    app.extract_ticked_pieces(split);
    assert_eq!(app.history.undo_len(), before, "extracting nothing recorded an undo step");

    app.tick_piece(pieces[0], false);
    app.tick_piece(pieces[3], true);
    app.extract_ticked_pieces(split);
    let rows = crate::panel_outliner::visible_rows(&app);
    assert!(rows.contains(&pieces[0]) && rows.contains(&pieces[3]), "the extracted pieces got no rows");
    assert!(!rows.contains(&pieces[1]), "a piece nobody asked for was extracted too");
    assert!(app.scene.is_collection(split), "extracting two of eight dissolved the collection");
    // The ticks survive the extraction, so Put back is the way straight
    // back: clearing them left that button greyed out the moment anything
    // had been extracted, and the way back was to find the same pieces in
    // the list and tick them again.
    assert_eq!(app.piece_ticks.len(), 2, "the extraction cleared the ticks");

    // And they fold back in, without having to be found again.
    app.return_ticked_pieces(split);
    let rows = crate::panel_outliner::visible_rows(&app);
    assert!(!rows.contains(&pieces[0]) && !rows.contains(&pieces[3]), "the pieces kept their rows");

    // One undo per step, and the first one puts both rows away again.
    app.run(Command::Undo);
    app.run(Command::Undo);
    assert!(app.scene.row_children(split).is_empty(), "undo left the pieces in the tree");
}

#[test]
pub(crate) fn extracting_every_piece_asks_before_it_empties_the_collection() {
    // It is the one step that is not reversible by the feature itself: with
    // nothing left inside it the collection is a union group, and the shape
    // it was cut from goes with it (issue 82).
    let mut app = headless_app();
    split_with(&mut app, simple3d_geom::tiling::Tiling { size: 10.0, ..Default::default() });
    let split = app.primary().unwrap();
    let pieces = app.scene.node(split).children.clone();

    // Ticking every one of them is the same question, however it is asked.
    for &piece in &pieces {
        app.tick_piece(piece, true);
    }
    let before = app.history.undo_len();
    app.extract_ticked_pieces(split);
    assert_eq!(app.modal, Modal::ConfirmExtractAll, "emptying the collection went through unasked");
    assert_eq!(app.history.undo_len(), before, "it edited the document before the question was answered");
    assert!(app.scene.is_collection(split));

    app.extract_all_pieces(split);
    assert!(!app.scene.is_collection(split), "the collection survived being emptied");
    assert_eq!(app.scene.node(split).group_op(), Some(GroupOp::Union), "what is left is not a union group");
    assert_eq!(app.scene.node(split).name, "Plate", "the group is not named what the collection was");
    let rows = crate::panel_outliner::visible_rows(&app);
    assert!(pieces.iter().all(|p| rows.contains(p)), "the pieces did not become ordinary rows");

    // And one undo puts the collection back, recipe and all.
    app.run(Command::Undo);
    assert!(app.scene.node(split).is_split());
    assert!(app.scene.node(split).split_original().is_some());
}

#[test]
pub(crate) fn a_tick_belongs_to_the_collection_it_was_made_in() {
    // Ticks are not a selection, and they must not outlive the panel that
    // shows them: extracting into a collection the user has moved on from is
    // an edit somewhere they are not looking.
    let mut app = headless_app();
    split_with(&mut app, simple3d_geom::tiling::Tiling { size: 10.0, ..Default::default() });
    let split = app.primary().unwrap();
    let piece = app.scene.node(split).children[0];
    app.tick_piece(piece, false);
    assert_eq!(app.listed_collection(), Some(split));

    let root = app.scene.root();
    app.select_only(root);
    assert!(app.piece_ticks.is_empty(), "the ticks survived the selection moving off the collection");
    assert_eq!(app.listed_collection(), None);
}

#[test]
pub(crate) fn clicking_a_ticked_piece_again_unticks_it() {
    let mut app = headless_app();
    split_with(&mut app, simple3d_geom::tiling::Tiling { size: 10.0, ..Default::default() });
    let split = app.primary().unwrap();
    let pieces = app.scene.node(split).children.clone();

    app.tick_piece(pieces[0], false);
    assert_eq!(app.piece_ticks.len(), 1);
    app.tick_piece(pieces[0], false);
    assert!(app.piece_ticks.is_empty(), "a second click on the same piece left it ticked");
    // A plain click replaces what was ticked; Ctrl adds to it.
    app.tick_piece(pieces[0], false);
    app.tick_piece(pieces[1], false);
    assert_eq!(app.piece_ticks.len(), 1, "a plain click added rather than replacing");
    app.tick_piece(pieces[2], true);
    assert_eq!(app.piece_ticks.len(), 2);
}
