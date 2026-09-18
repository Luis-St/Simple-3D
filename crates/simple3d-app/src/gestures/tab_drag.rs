//! Dragging a tab off its row, and clicking one (issue 107).
//!
//! The same widget carries both gestures now, so the thing that can go wrong is
//! that it cannot tell them apart -- and the drag has to be claimed by the tab
//! rather than by the viewport it is dragged over.

use super::*;
use crate::shell::WindowRequest;
use crate::tabs::strip::tab_id;

/// Two documents, so a tab pulled out of the row is one of several.
fn two_tabs(name: &str) -> Harness<'static, App> {
    let mut harness = harness_configured(name, |app| {
        app.new_project();
    });
    harness.step();
    harness
}

#[test]
fn dragging_a_tab_off_the_row_asks_for_a_window_of_its_own() {
    let mut harness = two_tabs("tab-drag-out");
    assert_eq!(harness.state().tab_count(), 2);
    let tab = rect_of(&harness, tab_id(0));

    press(&mut harness, tab.center());
    let pulled = tab.center() + egui::vec2(10.0, 80.0);
    move_to(&mut harness, pulled);
    let drag = harness.state().tab_drag.expect("the tab did not claim the drag");
    assert_eq!(drag.tab, 0);
    assert!(!drag.whole_window, "one tab was dragged and the whole window came with it");
    release(&mut harness, pulled);

    assert_eq!(
        harness.state().window_request,
        Some(WindowRequest::Detach(0)),
        "the tab was let go off the row and did not ask for a window"
    );
    assert!(harness.state().tab_drag.is_none(), "the drag was left running after the button came up");
}

#[test]
fn a_tab_dragged_along_its_own_row_stays_where_it_is() {
    let mut harness = two_tabs("tab-drag-along");
    let first = rect_of(&harness, tab_id(0));
    let second = rect_of(&harness, tab_id(1));

    press(&mut harness, first.center());
    move_to(&mut harness, second.center());
    release(&mut harness, second.center());

    assert_eq!(harness.state().window_request, None, "a drag along the row moved the document out of the window");
    assert_eq!(harness.state().tab_count(), 2);
}

#[test]
fn clicking_a_tab_still_shows_it() {
    let mut harness = two_tabs("tab-click");
    assert_eq!(harness.state().active, 1, "a new document is not the one on screen");
    let first = rect_of(&harness, tab_id(0));

    press(&mut harness, first.center());
    release(&mut harness, first.center());

    assert_eq!(harness.state().active, 0, "a click on a tab did not show it");
    assert_eq!(harness.state().window_request, None, "a click was taken for a drag out of the window");
}

/// The empty part of the row carries the whole window, and only when there is
/// another window to carry it to: with one window open there is nowhere for it
/// to go, so the gesture does not start at all.
#[test]
fn the_row_itself_carries_nothing_while_there_is_only_one_window() {
    let mut harness = two_tabs("tab-strip-alone");
    let rest = rect_of(&harness, crate::tabs::strip::rest_id());

    press(&mut harness, rest.center());
    move_to(&mut harness, rest.center() + egui::vec2(0.0, 80.0));

    assert!(harness.state().tab_drag.is_none(), "the row was carried with nowhere to carry it to");
    release(&mut harness, rest.center() + egui::vec2(0.0, 80.0));
    assert_eq!(harness.state().window_request, None);
}
