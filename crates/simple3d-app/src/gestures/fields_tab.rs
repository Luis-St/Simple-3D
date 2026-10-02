//! Tab and Shift+Tab walking from one value field to the next.

use super::*;

/// A typed key as a window sends it: the press and, for a printable key, its text in the same frame.
fn type_key(harness: &mut Harness<'_, App>, key: egui::Key, text: &str) {
    let modifiers = harness.input().modifiers;
    let input = harness.input_mut();
    input.events.push(egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
    input.events.push(egui::Event::Text(text.to_string()));
    harness.step();
    event(harness, egui::Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers });
}

/// Regression: Tab out of Position X left no field focused, so the next digit typed reached the
/// keymap (5 toggled the grid) instead of Position Y.
#[test]
pub(crate) fn tab_in_a_value_field_commits_it_and_types_into_the_next_one() {
    let mut harness = harness("field-tab");
    let plate = harness.state().primary().expect("the starter shape is selected");
    let grid = harness.state().scene.settings.grid_visible;

    let field = rect_of(&harness, crate::panel_properties::grip_id("Position (mm):0"));
    press(&mut harness, field.center());
    release(&mut harness, field.center());
    harness.step();
    text(&mut harness, "25");
    key(&mut harness, egui::Key::Tab);
    type_key(&mut harness, egui::Key::Num5, "5");
    key(&mut harness, egui::Key::Enter);
    harness.step();

    let app = harness.state();
    assert_eq!(app.scene.node(plate).position.x, 25.0, "Tab did not commit the typed X");
    assert_eq!(app.scene.node(plate).position.y, 5.0, "the 5 typed after Tab did not reach Position Y");
    assert_eq!(app.scene.settings.grid_visible, grid, "a digit typed into a field toggled the grid");
    assert!(!app.settings.layout.docks_hidden, "Tab out of a field also hid the docks");
}

/// Shift+Tab walks back: from Y to X.
#[test]
pub(crate) fn shift_tab_in_a_value_field_types_into_the_one_before() {
    let mut harness = harness("field-shift-tab");
    let plate = harness.state().primary().expect("the starter shape is selected");

    let field = rect_of(&harness, crate::panel_properties::grip_id("Position (mm):1"));
    press(&mut harness, field.center());
    release(&mut harness, field.center());
    harness.step();
    text(&mut harness, "7");
    modifiers(&mut harness, egui::Modifiers::SHIFT);
    key(&mut harness, egui::Key::Tab);
    modifiers(&mut harness, egui::Modifiers::NONE);
    type_key(&mut harness, egui::Key::Num3, "3");
    key(&mut harness, egui::Key::Enter);
    harness.step();

    let app = harness.state();
    assert_eq!(app.scene.node(plate).position.y, 7.0, "Shift+Tab did not commit the typed Y");
    assert_eq!(app.scene.node(plate).position.x, 3.0, "the 3 typed after Shift+Tab did not reach Position X");
    assert!(!app.settings.layout.docks_hidden, "Shift+Tab out of a field also hid the docks");
}
