//! Dragging a shape out of the palette into the tree.

use super::*;

#[test]
pub(crate) fn a_shape_is_dragged_out_of_the_palette_and_dropped_into_the_tree() {
    // The palette's tiles add a shape at the document's insertion point when
    // they are clicked. Dragged, they carry the shape into the outliner and the
    // drop says which row it belongs on -- the same gesture, the same slab on
    // the pointer and the same drop indicator as dragging a row already there.
    let mut harness = harness("palette-drag");
    let root = harness.state().scene.root();
    let plate = harness.state().primary().expect("the starter shape is selected");
    let group = harness.state_mut().scene.add_group(simple3d_core::scene::GroupOp::Union, root, 1);
    harness.state_mut().select_only(plate);
    harness.step();
    harness.step();
    let before = harness.state().scene.node(group).children.len();

    let tile = rect_of(&harness, crate::panel_primitives::tile_id("sphere")).center();
    press(&mut harness, tile);
    move_to(&mut harness, tile + egui::vec2(0.0, 6.0));
    // Settled over several frames: a response reports the frame it was drawn
    // in, so the press only becomes a drag a frame or two after the move.
    for _ in 0..4 {
        harness.step();
    }
    assert_eq!(
        harness.state().outliner_drag,
        Some(crate::app::Carried::Shape("sphere")),
        "dragging a tile did not pick the shape up"
    );

    // Over the group, held for several frames, because a response reports the
    // frame it was drawn in.
    for _ in 0..3 {
        let onto = rect_of(&harness, crate::panel_outliner::row_id(group)).center();
        move_to(&mut harness, onto);
        harness.step();
    }
    let target = harness.state().drop_target.expect("a shape from the palette marked no drop target");
    assert_eq!(target.into, Some(group), "the group under the pointer was not marked as what would take the drop");

    let onto = rect_of(&harness, crate::panel_outliner::row_id(group)).center();
    release(&mut harness, onto);
    harness.step();
    let app = harness.state();
    assert_eq!(app.scene.node(group).children.len(), before + 1, "the shape did not land in the group");
    let landed = *app.scene.node(group).children.last().unwrap();
    assert_eq!(app.scene.node(landed).spec().map(|s| s.type_id), Some("sphere"), "the wrong shape landed");
    assert_eq!(app.primary(), Some(landed), "the dropped shape was not left selected");
    assert!(app.outliner_drag.is_none() && app.drop_target.is_none(), "the drag outlived the drop");

    // And it undoes in one step, like every other add.
    harness.state_mut().run(simple3d_core::keymap::Command::Undo);
    harness.step();
    assert_eq!(harness.state().scene.node(group).children.len(), before);
}

#[test]
pub(crate) fn a_tile_still_adds_its_shape_when_it_is_merely_clicked() {
    // The tile answers to both gestures, and teaching it to drag must not have
    // cost it the click it had before.
    let mut harness = harness("palette-click");
    let root = harness.state().scene.root();
    let before = harness.state().scene.node(root).children.len();

    let tile = rect_of(&harness, crate::panel_primitives::tile_id("sphere")).center();
    press(&mut harness, tile);
    release(&mut harness, tile);
    harness.step();
    let app = harness.state();
    assert_eq!(app.scene.node(root).children.len(), before + 1, "a click on a tile added nothing");
    assert!(app.outliner_drag.is_none(), "a click left a drag running");
}

#[test]
pub(crate) fn a_dialog_stops_the_mouse_as_well_as_the_keyboard() {
    // With About open, clicking a tile in the palette still added a shape,
    // while Ctrl+N did nothing: `handle_shortcuts` returned early on a modal
    // and nothing stopped the pointer. A dialog left open behind the main
    // window was an application whose shortcuts had silently stopped while the
    // document could still be edited by mouse.
    let mut harness = harness("dialog-blocks-pointer");
    let root = harness.state().scene.root();
    let before = harness.state().scene.node(root).children.len();
    let tile = rect_of(&harness, crate::panel_primitives::tile_id("sphere")).center();

    harness.state_mut().modal = crate::app::Modal::About;
    // Two frames: the backdrop has to be drawn once before its layer can win a
    // hit test, which egui settles at the end of the pass it was drawn in.
    harness.step();
    harness.step();

    press(&mut harness, tile);
    release(&mut harness, tile);
    harness.step();
    assert_eq!(
        harness.state().scene.node(root).children.len(),
        before,
        "a click reached the palette through an open dialog"
    );

    harness.state_mut().modal = crate::app::Modal::None;
    harness.step();
    harness.step();
    press(&mut harness, tile);
    release(&mut harness, tile);
    harness.step();
    assert_eq!(
        harness.state().scene.node(root).children.len(),
        before + 1,
        "closing the dialog did not give the palette its clicks back"
    );
}
