//! What happens to the window when the pattern goes.

use super::*;

/// Deleting a pattern from the tree took the window with it.
///
/// The row list is taken before any of it is drawn, and the context menu's
/// Delete removes nodes while the loop over that list is still running. A
/// pattern is not a *group*, so deleting one asks nothing and goes at once --
/// and `Scene::remove` takes the whole subtree, which is exactly the rows that
/// come next. The next row drawn then asked the scene for a node that was no
/// longer in it, and `Scene::node` panics on that.
///
/// Driven through the real tree because that is the only place the fault
/// existed: every piece of it -- the list, the menu, the delete -- is correct on
/// its own, and it is the order they run in that was wrong.
#[test]
pub(crate) fn deleting_a_pattern_from_the_tree_does_not_take_the_window_with_it() {
    use egui_kittest::kittest::Queryable;

    let mut harness = harness("outliner-delete-pattern");
    let plate = harness.state().primary().expect("the starting scene has a plate selected");
    // A pattern *of* the plate, so the tree is Scene > Pattern > Plate and the
    // rows below the one being deleted are the ones that go with it.
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

    // The menu's Delete carries its binding after a tab, so it is found by what
    // it starts with rather than by the whole label.
    let delete = harness
        .query_all_by_label_contains("Delete")
        .next()
        .expect("the tree's context menu has no Delete on a pattern");
    delete.click();
    // The frame the deletion happens on is the frame that used to panic: the
    // rows after the pattern are its children, and they are gone by then.
    harness.step();
    harness.step();

    assert!(!harness.state().scene.contains(pattern), "the pattern is still there");
    assert!(!harness.state().scene.contains(plate), "the pattern went and left its child behind");
    assert!(harness.state().selection.is_empty(), "the deleted node is still selected");
}
