//! What happens to the window when the pattern goes.

use super::*;

/// Regression: deleting a pattern from its row menu removed its children mid-loop over the row list,
/// and the next row's `Scene::node` panicked. Driven through the real tree, since each piece was
/// correct alone and only their order was wrong.
#[test]
pub(crate) fn deleting_a_pattern_from_the_tree_does_not_take_the_window_with_it() {
    use egui_kittest::kittest::Queryable;

    let mut harness = harness("outliner-delete-pattern");
    let plate = harness.state().primary().expect("the starting scene has a plate selected");
    // A pattern of the plate, so the rows after the deleted one go with it.
    harness.state_mut().run(simple3d_core::keymap::Command::Pattern);
    harness.step();
    let pattern = harness.state().primary().expect("the plate is now inside a pattern");
    assert!(harness.state().scene.node(pattern).is_pattern());
    assert_eq!(harness.state().scene.node(pattern).children, vec![plate], "the plate is not in the pattern");

    let row = rect_of(&harness, crate::panel_outliner::row_id(pattern));
    let at = row.center();
    move_to(&mut harness, at);
    button(&mut harness, at, egui::PointerButton::Secondary, true);
    button(&mut harness, at, egui::PointerButton::Secondary, false);
    harness.step();
    harness.step();

    // Delete's label carries its binding after a tab, so it is found by prefix.
    let delete = harness
        .query_all_by_label_contains("Delete")
        .next()
        .expect("the tree's context menu has no Delete on a pattern");
    delete.click();
    // The frame the deletion happens on is the one that used to panic.
    harness.step();
    harness.step();

    assert!(!harness.state().scene.contains(pattern), "the pattern is still there");
    assert!(!harness.state().scene.contains(plate), "the pattern went and left its child behind");
    assert!(harness.state().selection.is_empty(), "the deleted node is still selected");
}
