//! Clicking into the viewport while a shape is set up in the add popup.

use super::*;

/// A click near the viewport's bottom-left corner, clear of the popup over its middle.
fn click_corner(harness: &mut Harness<'_, App>) {
    let viewport = harness.state().viewport_rect;
    let at = viewport.left_bottom() + egui::vec2(30.0, -30.0);
    press(harness, at);
    release(harness, at);
    harness.step();
}

#[test]
pub(crate) fn a_click_into_the_viewport_leaves_the_shape_being_set_up_selected() {
    let mut harness = harness("add-popup-click");
    harness.state_mut().open_add_popup("box");
    harness.step();
    let id = harness.state().adding().expect("the popup did not open");

    click_corner(&mut harness);
    assert!(harness.state().add_popup.is_some(), "the click closed the popup");
    assert_eq!(harness.state().selection, vec![id], "the click changed the selection");

    // The same click with nothing being set up does deselect, so the test clicks where it means to.
    harness.state_mut().confirm_add_popup();
    harness.step();
    click_corner(&mut harness);
    assert!(harness.state().selection.is_empty(), "the corner click selected something");
}
