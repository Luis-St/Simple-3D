//! The menu a right click opens on a row.

use super::*;

// -- the outliner's context menu ----------------------------------------------

#[test]
pub(crate) fn right_clicking_an_unselected_outliner_row_opens_its_menu_at_the_first_press() {
    // Regression: the menu selects its row, which renumbered it, so the menu was looked up under a
    // stale id and closed the next frame.
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
    // Two more frames: the menu opens the frame after the click, and the one after that lost it.
    harness.step();
    harness.step();

    assert!(harness.state().is_selected(cube), "the right-click did not select the row it was on");
    assert!(
        egui::Popup::is_id_open(&harness.ctx, crate::panel_outliner::row_id(cube).with("popup")),
        "the first right-click on an unselected row did not leave its context menu open"
    );
}

/// Issue 67: the tree's Add menu offers the custom-pattern tool like the menu bar's; the two had
/// drifted apart.
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

    // Find the tree's Add submenu, not the menu bar's, which a plain label lookup finds first (and let
    // this test pass against the bug).
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
    assert!(harness.state().pattern_tool.is_some(), "the entry did not open the tool");
    let opened = harness.state().pattern_tool.expect("the tool opened on nothing");
    assert!(harness.state().scene.node(opened).is_pattern(), "the tool opened on something that is not a pattern");
}
