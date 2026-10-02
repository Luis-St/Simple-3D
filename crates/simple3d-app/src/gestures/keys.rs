//! Chords that reach the application, modifiers included.

use super::*;

#[test]
pub(crate) fn the_clipboard_chords_reach_the_application() {
    // `egui-winit` turns Ctrl+X/C/V into Cut/Copy/Paste without emitting the key, so these bindings can
    // only be tested through real window events; driving `Command::Copy` directly missed it.
    let mut harness = harness("clipboard-chords");
    let root = harness.state().scene.root();
    let before = harness.state().scene.node(root).children.len();

    event(&mut harness, egui::Event::Copy);
    assert!(harness.state().clipboard.is_some(), "Ctrl+C filled the clipboard");

    event(&mut harness, egui::Event::Paste("anything".into()));
    assert_eq!(harness.state().scene.node(root).children.len(), before + 1, "Ctrl+V pasted the copied node");

    event(&mut harness, egui::Event::Cut);
    assert_eq!(harness.state().scene.node(root).children.len(), before, "Ctrl+X took the node away");
}

// -- issue 77: a modifier on its own is a binding ------------------------------

#[test]
pub(crate) fn a_modifier_released_on_its_own_fires_what_it_is_bound_to() {
    // Issue 77: modifiers alone were unbindable. Driven through the window, since modifier presses
    // are read from frame-to-frame state.
    let mut harness = harness("modifier-only-binding");
    harness
        .state_mut()
        .keymap
        .set(
            simple3d_core::keymap::Command::ToggleGrid,
            simple3d_core::keymap::Chord::modifiers(false, false, true),
            true,
        )
        .unwrap();
    let before = harness.state().scene.settings.grid_visible;

    // Alt down for a couple of frames, then up with nothing under it.
    modifiers(&mut harness, egui::Modifiers::ALT);
    harness.step();
    harness.step();
    assert_eq!(harness.state().scene.settings.grid_visible, before, "the binding fired while the key was still down");
    modifiers(&mut harness, egui::Modifiers::NONE);
    harness.step();
    assert_ne!(harness.state().scene.settings.grid_visible, before, "releasing the modifier did not fire its binding");

    // A modifier held under another key is a combination, and must not also fire its own binding.
    let grid = harness.state().scene.settings.grid_visible;
    modifiers(&mut harness, egui::Modifiers::ALT);
    key(&mut harness, egui::Key::J);
    modifiers(&mut harness, egui::Modifiers::NONE);
    harness.step();
    assert_eq!(harness.state().scene.settings.grid_visible, grid, "a combination fired the modifier binding as well");
}

#[test]
pub(crate) fn several_keys_held_together_fire_the_combination_they_make() {
    // Any key can base a combination: Q+W+E fires on the completing press.
    let mut harness = harness("combination-binding");
    harness
        .state_mut()
        .keymap
        .set(simple3d_core::keymap::Command::ToggleGrid, simple3d_core::keymap::Chord::combo(["Q", "W", "E"]), true)
        .unwrap();
    let before = harness.state().scene.settings.grid_visible;

    // Held one after another; `keys_down` is raw input, so each stays down until released.
    hold_key(&mut harness, egui::Key::Q, true);
    hold_key(&mut harness, egui::Key::W, true);
    assert_eq!(harness.state().scene.settings.grid_visible, before, "an incomplete combination fired");
    hold_key(&mut harness, egui::Key::E, true);
    assert_ne!(harness.state().scene.settings.grid_visible, before, "the completing press did not fire it");

    let after = harness.state().scene.settings.grid_visible;
    for key in [egui::Key::Q, egui::Key::W, egui::Key::E] {
        hold_key(&mut harness, key, false);
    }
    assert_eq!(harness.state().scene.settings.grid_visible, after, "letting go fired it a second time");
}

#[test]
pub(crate) fn a_modifier_held_over_a_click_is_the_click_not_a_binding() {
    // Ctrl+click adds to the selection without firing Ctrl's own binding.
    let mut harness = harness("modifier-only-click");
    harness
        .state_mut()
        .keymap
        .set(
            simple3d_core::keymap::Command::ToggleGrid,
            simple3d_core::keymap::Chord::modifiers(true, false, false),
            true,
        )
        .unwrap();
    let before = harness.state().scene.settings.grid_visible;

    let viewport = harness.state().viewport_rect.center();
    modifiers(&mut harness, egui::Modifiers::COMMAND);
    press(&mut harness, viewport);
    release(&mut harness, viewport);
    modifiers(&mut harness, egui::Modifiers::NONE);
    harness.step();
    assert_eq!(harness.state().scene.settings.grid_visible, before, "a Ctrl+click also fired the Ctrl binding");
}

/// Regression: Align and distribute and Snap to geometry had no marker slot, so their words
/// started left of the four modes' in the Manipulate menu.
#[test]
pub(crate) fn every_manipulate_menu_entry_keeps_the_marker_slot() {
    use egui_kittest::kittest::Queryable;
    let mut harness = harness("manipulate-menu");
    harness.get_by_label("Manipulate").click();
    harness.step();
    harness.step();
    for label in ["* Move", "  Rotate", "  Align and distribute", "  Snap to geometry"] {
        assert!(harness.query_by_label_contains(label).is_some(), "no entry reads {label:?}");
    }
}
