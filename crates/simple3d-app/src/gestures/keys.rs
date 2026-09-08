//! Chords that reach the application, modifiers included.

use super::*;

#[test]
pub(crate) fn the_clipboard_chords_reach_the_application() {
    // `egui-winit` turns Ctrl+X, Ctrl+C and Ctrl+V into `Cut`, `Copy` and
    // `Paste` and never emits the key press underneath them, so these three
    // bindings -- alone in the whole keymap -- can only be tested through the
    // events the window system really delivers. Driving `Command::Copy`
    // directly, which is what the other tests do, is exactly what let this go
    // unnoticed.
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
    // Issue 77: Ctrl, Shift and Alt could not be bound at all, because a chord
    // needed a key beside them. Driven through the window because that is where
    // the rule lives: the toolkit reports no key event for a modifier, so the
    // press has to be read off the modifier state frame by frame.
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

    // Alt down for a couple of frames, then up with nothing pressed under it.
    modifiers(&mut harness, egui::Modifiers::ALT);
    harness.step();
    harness.step();
    assert_eq!(harness.state().scene.settings.grid_visible, before, "the binding fired while the key was still down");
    modifiers(&mut harness, egui::Modifiers::NONE);
    harness.step();
    assert_ne!(harness.state().scene.settings.grid_visible, before, "releasing the modifier did not fire its binding");

    // The same modifier held under another key is a combination, and firing that
    // must not also fire the modifier's own binding on the way out.
    let grid = harness.state().scene.settings.grid_visible;
    modifiers(&mut harness, egui::Modifiers::ALT);
    key(&mut harness, egui::Key::J);
    modifiers(&mut harness, egui::Modifiers::NONE);
    harness.step();
    assert_eq!(harness.state().scene.settings.grid_visible, grid, "a combination fired the modifier binding as well");
}

#[test]
pub(crate) fn several_keys_held_together_fire_the_combination_they_make() {
    // Any key can be the base of a combination, not only a modifier: Q+W+E is a
    // binding, and it fires on the press that completes it.
    let mut harness = harness("combination-binding");
    harness
        .state_mut()
        .keymap
        .set(simple3d_core::keymap::Command::ToggleGrid, simple3d_core::keymap::Chord::combo(["Q", "W", "E"]), true)
        .unwrap();
    let before = harness.state().scene.settings.grid_visible;

    // Held down one after another, the way a hand performs it. `keys_down` is
    // raw-input state, so each key stays down until it is let go.
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
    // Ctrl+click adds to the selection, and must not also fire whatever Ctrl
    // alone is bound to when the hand comes off the key.
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
