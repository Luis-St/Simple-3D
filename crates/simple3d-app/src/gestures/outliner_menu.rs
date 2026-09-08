//! The menu a right click opens on a row.

use super::*;

// -- the outliner's context menu ----------------------------------------------

#[test]
pub(crate) fn right_clicking_an_unselected_outliner_row_opens_its_menu_at_the_first_press() {
    // The menu selects the row it was opened on, and that selection used to
    // renumber the row underneath it, so the menu was looked for under an id
    // nothing had drawn and closed again on the very next frame.
    let mut harness = harness("outliner-context-menu");
    let root = harness.state().scene.root();
    let cube = harness.state_mut().scene.add_primitive("box", root, 1).expect("the box is in the registry");
    harness.step();
    assert!(!harness.state().is_selected(cube), "the row this test right-clicks was already selected");

    let row = rect_of(&harness, crate::panel_outliner::row_id(cube));
    let at = egui::pos2(row.center().x, row.center().y);
    move_to(&mut harness, at);
    button(&mut harness, at, egui::PointerButton::Secondary, true);
    button(&mut harness, at, egui::PointerButton::Secondary, false);
    // Two more frames: the menu is opened out of memory on the frame after the
    // click, and it is the frame after *that* which used to lose it.
    harness.step();
    harness.step();

    assert!(harness.state().is_selected(cube), "the right-click did not select the row it was on");
    assert!(
        egui::Popup::is_id_open(&harness.ctx, crate::panel_outliner::row_id(cube).with("popup")),
        "the first right-click on an unselected row did not leave its context menu open"
    );
}

/// Issue 67: the tree's own Add menu offers the custom-kind tool, the way the
/// menu bar's does.
///
/// The two Add menus are meant to hold the same things and differ only in where
/// what they add lands, and they drifted: "Custom pattern" went into the menu
/// bar's only, so the tree -- which is where a container is most often added
/// from -- offered "Pattern" and "Make a pattern of the selection" and no way at
/// all to reach the tool that builds a rule. A feature nobody can find is a
/// feature nobody has.
#[test]
pub(crate) fn the_tree_add_menu_offers_the_custom_pattern_tool() {
    use egui_kittest::kittest::Queryable;

    let mut harness = harness("outliner-add-custom-pattern");
    let root = harness.state().scene.root();
    let cube = harness.state_mut().scene.add_primitive("box", root, 1).expect("the box is in the registry");
    harness.step();

    let row = rect_of(&harness, crate::panel_outliner::row_id(cube));
    let at = row.center();
    move_to(&mut harness, at);
    button(&mut harness, at, egui::PointerButton::Secondary, true);
    button(&mut harness, at, egui::PointerButton::Secondary, false);
    harness.step();
    harness.step();

    // The menu bar has an "Add" of its own, and it is the one a plain lookup by
    // label finds -- which is how this test first passed against the very code it
    // was written to catch. The tree's is a submenu, so its label carries the
    // arrow egui puts on one, and it is drawn well below the menu bar.
    let add = harness
        .query_all_by_label_contains("Add")
        .find(|node| node.rect().center().y > 40.0)
        .expect("the tree's context menu has no Add of its own");
    add.click();
    harness.step();
    harness.step();
    assert!(
        harness.query_by_label("Custom pattern").is_some(),
        "the tree's Add menu has no way through to the custom pattern kind tool"
    );

    harness.get_by_label("Custom pattern").click();
    harness.step();
    harness.step();
    assert_eq!(harness.state().modal, crate::app::Modal::PatternKind, "the entry did not open the tool");
    let opened = harness.state().pattern_tool.expect("the tool opened on nothing");
    assert!(harness.state().scene.node(opened).is_pattern(), "the tool opened on something that is not a pattern");
}
