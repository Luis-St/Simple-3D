//! Turning a key press into a command.

use crate::app::{App, Modal};
use crate::ui;
use simple3d_core::keymap::Chord;

impl App {
    /// Run whatever command the pressed keys are bound to. Text fields keep the
    /// keyboard when they have focus, so typing a dimension never fires a
    /// shortcut.
    pub(crate) fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if self.modal != Modal::None || self.recording.is_some() || self.rename.is_some() {
            // The release that ends a modifier hold will be delivered somewhere
            // else, so the hold in progress is not one this window can finish.
            self.shortcut_mods.reset();
            return;
        }
        if ctx.wants_keyboard_input() {
            self.shortcut_mods.reset();
            return;
        }
        // Escape puts an in-place popup away. A popup is not modal, so nothing
        // else was going to catch the press -- and a window with no dialog
        // machinery behind it still has to answer the key that closes windows
        // (issue 82). Taken before the keymap, so a binding on Escape does not
        // fire in the same breath as the tool it would be closing.
        if self.split_tool.is_some() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.cancel_split_tool();
            return;
        }
        let (events, modifiers, held, pointer) = ctx.input(|input| {
            // `egui-winit` eats Ctrl+X, Ctrl+C and Ctrl+V: it pushes `Cut`,
            // `Copy` or `Paste` and returns *before* it emits the `Key` event
            // (egui-winit 0.32.3 `lib.rs:766-781`). Nothing downstream ever
            // sees the press, so those three bindings -- and anything a user
            // rebinds onto them -- could never fire. Putting the press back is
            // what keeps the keymap the single answer to what a chord does,
            // rather than hard-wiring copy and paste past it.
            let events: Vec<(egui::Key, egui::Modifiers)> = input
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Key { key, pressed: true, modifiers, .. } => Some((*key, *modifiers)),
                    egui::Event::Cut => Some((egui::Key::X, egui::Modifiers::COMMAND)),
                    egui::Event::Copy => Some((egui::Key::C, egui::Modifiers::COMMAND)),
                    egui::Event::Paste(_) => Some((egui::Key::V, egui::Modifiers::COMMAND)),
                    _ => None,
                })
                .collect();
            let pointer = input.pointer.any_down() || input.pointer.any_pressed();
            (events, input.modifiers, ui::keys_down(input), pointer)
        });
        // A press fires the longest binding everything held down satisfies, so a
        // combination of ordinary keys -- Q+W+E -- fires on the key that
        // completes it rather than every key in it firing its own binding.
        for (key, modifiers) in events {
            let pressed = key.name();
            let down = |name: &str| name == pressed || held.iter().any(|k| k == name);
            let command =
                self.keymap.command_for_press(pressed, down, modifiers.command, modifiers.shift, modifiers.alt);
            if let Some(command) = command {
                self.run(command);
            }
        }
        // A modifier held on its own, and let go of with nothing pressed under
        // it, is a binding in its own right (issue 77) -- and only that case is
        // taken here, because a chord with keys in it has already fired on the
        // press that completed it. A mouse button counts as something pressed
        // under it: Ctrl+click picks a second object, and must not also fire
        // whatever Ctrl alone is bound to.
        let completed = self.shortcut_mods.update(modifiers, held.iter().map(String::as_str), pointer);
        if let Some(chord) = completed.filter(Chord::is_modifier_only) {
            if let Some(command) = self.keymap.command_for(&chord) {
                self.run(command);
            }
        }
    }
}
