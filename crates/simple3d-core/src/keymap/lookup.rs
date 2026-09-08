//! Finding the command a press means, and changing what a key does.

use super::*;

impl Keymap {
    pub fn binding(&self, command: Command) -> Option<&Chord> {
        self.bindings.get(&command)
    }

    /// The label to show wherever a shortcut appears -- menus, tooltips, help.
    /// Always the *current* binding, never a hardcoded string.
    pub fn shortcut_text(&self, command: Command) -> String {
        self.bindings.get(&command).map(|c| c.to_string()).unwrap_or_default()
    }

    pub fn command_for(&self, chord: &Chord) -> Option<Command> {
        self.bindings.iter().find(|(_, c)| *c == chord).map(|(k, _)| *k)
    }

    /// What a key press fires, given everything held down at that moment.
    ///
    /// The longest chord fully satisfied wins, so with Q and W already down,
    /// pressing E fires `Q+W+E` rather than whatever `E` alone is bound to. Only
    /// a chord containing the key just pressed is considered: a combination
    /// fires on the press that completes it, not again on every key afterwards.
    ///
    /// A key that is *part* of a longer combination still fires its own binding
    /// on its own press -- nothing can know a longer one is coming -- so a
    /// combination is worth building out of keys that are otherwise free.
    pub fn command_for_press(
        &self,
        pressed: &str,
        held: impl Fn(&str) -> bool,
        ctrl: bool,
        shift: bool,
        alt: bool,
    ) -> Option<Command> {
        self.bindings
            .iter()
            .filter(|(_, chord)| chord.keys.iter().any(|k| k == pressed))
            .filter(|(_, chord)| chord.satisfied_by(&held, ctrl, shift, alt))
            .max_by_key(|(_, chord)| chord.keys.len())
            .map(|(command, _)| *command)
    }

    /// Which command already holds `chord`, ignoring `command` itself. The
    /// keymap editor names it rather than silently overwriting.
    pub fn conflict(&self, command: Command, chord: &Chord) -> Option<Command> {
        self.bindings.iter().find(|(k, c)| **k != command && *c == chord).map(|(k, _)| *k)
    }

    /// Assign a binding. Refuses and names the holder on a conflict; the caller
    /// then offers to reassign (call again with `force`) or cancel.
    pub fn set(&mut self, command: Command, chord: Chord, force: bool) -> Result<(), Command> {
        if let Some(holder) = self.conflict(command, &chord) {
            if !force {
                return Err(holder);
            }
            self.bindings.remove(&holder);
        }
        self.bindings.insert(command, chord);
        Ok(())
    }

    pub fn unbind(&mut self, command: Command) {
        self.bindings.remove(&command);
    }

    /// Reset one binding to this keymap's preset default.
    pub fn reset(&mut self, command: Command) {
        let preset = Keymap::from_preset(self.preset);
        match preset.bindings.get(&command) {
            Some(chord) => {
                // Clear whoever holds it now, so the reset cannot introduce a
                // conflict of its own.
                if let Some(holder) = self.conflict(command, chord) {
                    self.bindings.remove(&holder);
                }
                self.bindings.insert(command, chord.clone());
            }
            None => {
                self.bindings.remove(&command);
            }
        }
    }

    pub fn reset_all(&mut self) {
        *self = Keymap::from_preset(self.preset);
    }

    pub fn switch_preset(&mut self, preset: Preset) {
        *self = Keymap::from_preset(preset);
    }

    /// Commands sharing a chord. Should always be empty; used by tests and as a
    /// sanity check when importing a hand-edited keymap file.
    pub fn self_conflicts(&self) -> Vec<(Command, Command)> {
        let mut out = Vec::new();
        let entries: Vec<(&Command, &Chord)> = self.bindings.iter().collect();
        for (i, (a, ca)) in entries.iter().enumerate() {
            for (b, cb) in &entries[i + 1..] {
                if ca == cb {
                    out.push((**a, **b));
                }
            }
        }
        out
    }
}
