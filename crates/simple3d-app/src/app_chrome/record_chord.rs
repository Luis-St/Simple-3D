//! Recording a shortcut, for the keymap editor and the palette's tiles (issue 124).

use crate::app::App;
use crate::ui;
use simple3d_core::keymap::{Action, Chord};
use simple3d_core::scene::GroupOp;

impl App {
    /// Feed `ctx`'s keys to the recording in progress. The binding is whatever is held together, taken
    /// on release (issue 77), which is what makes chords like Q+W+E possible; Escape cancels. Keys are
    /// read from the context of the window that started it, since the keymap dialog is its own window.
    pub(crate) fn record_chord(&mut self, ctx: &egui::Context) {
        let Some(target) = self.recording.clone() else { return };
        let (escaped, modifiers, held) = ctx.input(|input| {
            let escaped = input
                .events
                .iter()
                .any(|event| matches!(event, egui::Event::Key { key: egui::Key::Escape, pressed: true, .. }));
            (escaped, input.modifiers, ui::keys_down(input))
        });
        if escaped {
            self.recording = None;
            self.record_mods.reset();
            return;
        }
        let Some(chord) = self.record_mods.update(modifiers, held.iter().map(String::as_str), false) else { return };
        self.recording = None;
        self.bind_recorded(target, chord, false);
    }

    /// Bind `target` to `chord`; a chord someone else holds is kept for the user to reassign or cancel,
    /// never overwritten silently, unless `force`.
    pub(crate) fn bind_recorded(&mut self, target: Action, chord: Chord, force: bool) {
        match self.keymap.bind(&target, chord.clone(), force) {
            Ok(()) => {
                self.keymap_conflict = None;
                self.persist_keymap();
            }
            Err(holder) => self.keymap_conflict = Some((target, chord, holder)),
        }
    }

    /// Start recording a shortcut for `target`.
    pub(crate) fn start_recording(&mut self, target: Action) {
        self.recording = Some(target);
        self.keymap_conflict = None;
        // A modifier held for the click itself is not the binding.
        self.record_mods.reset();
    }

    /// Do what a shortcut is bound to.
    pub(crate) fn run_action(&mut self, action: Action) {
        match action {
            Action::Command(command) => self.run(command),
            Action::Primitive(type_id) => {
                if let Some(spec) = simple3d_core::primitive::lookup(&type_id) {
                    self.add_node(Some(spec.type_id), GroupOp::Union);
                }
            }
        }
    }
}
