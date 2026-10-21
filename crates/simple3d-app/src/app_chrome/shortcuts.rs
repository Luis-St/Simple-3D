//! Turning a key press into a command.

use crate::app::{App, Modal};
use crate::ui;
use simple3d_core::keymap::{Action, Chord, Command};

impl App {
    /// Run the command the pressed keys are bound to; focused text fields keep the keyboard.
    pub(crate) fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // A palette tile's recording (issue 124) takes the keys here; the keymap dialog reads its own.
        if self.modal == Modal::None && self.recording.is_some() {
            self.record_chord(ctx);
        }
        if self.modal != Modal::None
            || self.recording.is_some()
            || self.keymap_conflict.is_some()
            || self.rename.is_some()
        {
            // The release ending a modifier hold will land elsewhere, so drop the hold in progress.
            self.shortcut_mods.reset();
            self.cancel_repeat_hold();
            return;
        }
        // egui moves focus with Tab even though Tab is bound here, and a focused button then swallowed
        // every shortcut. Only text fields keep focus; any other is released.
        if let Some(id) = ctx.memory(|memory| memory.focused()) {
            if egui::text_edit::TextEditState::load(ctx, id).is_none() {
                ctx.memory_mut(|memory| memory.surrender_focus(id));
            }
        }
        if ctx.wants_keyboard_input() || self.fields.opening_one(ctx) {
            self.shortcut_mods.reset();
            self.cancel_repeat_hold();
            return;
        }
        // Escape closes an in-place popup, which being non-modal nothing else catches (issue 82). Taken
        // before the keymap so an Escape binding does not fire too.
        if self.split_tool.is_some() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.cancel_split_tool();
            return;
        }
        if self.arrange_tool.is_some() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.close_arrange_tool();
            return;
        }
        if self.mirror_tool.is_some() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.close_mirror_tool();
            return;
        }
        if self.cut_tool.is_some() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.close_cut_tool();
            return;
        }
        let (events, modifiers, held, pointer) = ctx.input(|input| {
            // `egui-winit` consumes Ctrl+X/C/V into Cut/Copy/Paste before emitting the key event (0.32.3
            // `lib.rs:766-781`), so the presses are restored to keep the keymap authoritative.
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
        // A press fires the longest binding satisfied by everything held, so Q+W+E fires once.
        for (key, modifiers) in events {
            let pressed = key.name();
            let down = |name: &str| name == pressed || held.iter().any(|k| k == name);
            let action = self.keymap.action_for_press(pressed, down, modifiers.command, modifiers.shift, modifiers.alt);
            // A shape's shortcut pressed with the add-with-settings key held opens its popup instead.
            // A command bound to the press itself still wins.
            if self.holding(Command::AddWithSettings, |key| down(key.name()), modifiers) {
                let shape = match &action {
                    Some(Action::Primitive(type_id)) => {
                        simple3d_core::primitive::lookup(type_id).map(|spec| spec.type_id)
                    }
                    None | Some(Action::Command(Command::AddWithSettings)) => {
                        self.primitive_under_settings_key(pressed, down, modifiers)
                    }
                    Some(Action::Command(_)) => None,
                };
                if let Some(type_id) = shape {
                    self.add_primitive(type_id, true);
                    continue;
                }
            }
            match action {
                // Previewed while held, and copied when let go (issue 121); the key's own repeats are ignored.
                Some(Action::Command(Command::RepeatDuplicate)) => {
                    if !self.repeat_hold {
                        self.start_repeat_hold();
                    }
                }
                Some(action) => self.run_action(action),
                None => {}
            }
        }
        let still = self.holding(Command::RepeatDuplicate, |key| held.iter().any(|k| k == key.name()), modifiers);
        self.follow_repeat_hold(still);
        // A modifier held and released alone is its own binding (issue 77); chords with keys already
        // fired. A mouse button counts as pressed, so Ctrl+click does not fire Ctrl.
        let completed = self.shortcut_mods.update(modifiers, held.iter().map(String::as_str), pointer);
        if let Some(chord) = completed.filter(Chord::is_modifier_only) {
            if let Some(action) = self.keymap.action_for(&chord) {
                self.run_action(action);
            }
        }
    }
}
