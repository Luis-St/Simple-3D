//! Dragging a panel between docks, and rolling one up.

use super::*;
use simple3d_core::config::{Panel, Side};

// -- the dock header: click rolls up, drag moves ------------------------------

#[test]
pub(crate) fn dragging_a_panel_header_into_the_other_dock_moves_the_panel_there() {
    let mut harness = harness("dock-drag");
    assert_eq!(harness.state().settings.layout.side_of(Panel::Primitives), Side::Left);

    let header = rect_of(&harness, crate::dock::header_id(Panel::Primitives));
    let properties = rect_of(&harness, crate::dock::header_id(Panel::Properties));
    // Above the Properties header at the right dock's top: drop index zero.
    let target = egui::pos2(properties.center().x, properties.top() + 2.0);

    press(&mut harness, header.center());
    move_to(&mut harness, header.center() + egui::vec2(0.0, 24.0));
    assert_eq!(
        harness.state().dock_drag.panel,
        Some(Panel::Primitives),
        "the header did not claim the drag -- something under it did"
    );
    move_to(&mut harness, target);
    assert_eq!(
        harness.state().dock_drag.target,
        Some((Side::Right, 0)),
        "the drag was over the right dock's first slot and did not say so"
    );
    release(&mut harness, target);

    let layout = &harness.state().settings.layout;
    assert_eq!(layout.side_of(Panel::Primitives), Side::Right, "the panel did not move dock");
    assert_eq!(layout.right, vec![Panel::Primitives, Panel::Properties], "it landed in the wrong place");
    assert_eq!(layout.left, vec![Panel::Outliner]);
    assert!(harness.state().dock_drag.panel.is_none(), "the drag was left running after the button came up");
    assert!(!layout.is_collapsed(Panel::Primitives), "a drag also rolled the panel up");

    // The panel is in the other dock now, drawn there on the next frame.
    harness.step();
    let moved = rect_of(&harness, crate::dock::header_id(Panel::Primitives));
    assert!(moved.center().x > 1400.0 / 2.0, "the panel is in the layout but is not drawn on the right");
}

#[test]
pub(crate) fn clicking_a_panel_header_rolls_it_up_without_moving_it() {
    // One widget carries click and drag, so they must be told apart.
    let mut harness = harness("dock-click");
    let header = rect_of(&harness, crate::dock::header_id(Panel::Outliner));
    let before = harness.state().settings.layout.clone();

    press(&mut harness, header.center());
    release(&mut harness, header.center());

    let layout = &harness.state().settings.layout;
    assert!(layout.is_collapsed(Panel::Outliner), "a click on the header did not roll the panel up");
    assert_eq!(layout.left, before.left, "a click moved the panel as well");
    assert_eq!(layout.right, before.right);

    // Clicking again unrolls it.
    let header = rect_of(&harness, crate::dock::header_id(Panel::Outliner));
    press(&mut harness, header.center());
    release(&mut harness, header.center());
    assert!(!harness.state().settings.layout.is_collapsed(Panel::Outliner), "the second click did not unroll it");
}

#[test]
pub(crate) fn tab_pressed_again_brings_the_docks_back() {
    // Regression: egui also moves focus with Tab, and a focused menu button swallowed later Tabs, so
    // the docks never came back.
    let mut harness = harness("docks-tab");
    key(&mut harness, egui::Key::Tab);
    assert!(harness.state().settings.layout.docks_hidden, "Tab did not hide the docks");
    key(&mut harness, egui::Key::Tab);
    assert!(!harness.state().settings.layout.docks_hidden, "the second Tab did not bring the docks back");
    key(&mut harness, egui::Key::Tab);
    assert!(harness.state().settings.layout.docks_hidden, "the third Tab did not hide them again");
    assert!(harness.ctx.memory(|memory| memory.focused()).is_none(), "Tab left the keyboard on a widget");
}
