//! Finding the command a press means, and changing what a key does.

use super::*;

impl Keymap {
    pub fn binding(&self, command: Command) -> Option<&Chord> {
        self.bindings.get(&command)
    }

    /// The label for a shortcut anywhere it appears: always the current binding.
    pub fn shortcut_text(&self, command: Command) -> String {
        self.bindings.get(&command).map(|c| c.to_string()).unwrap_or_default()
    }

    pub fn command_for(&self, chord: &Chord) -> Option<Command> {
        self.bindings.iter().find(|(_, c)| *c == chord).map(|(k, _)| *k)
    }

    /// What a key press fires given everything held: the longest satisfied chord containing the
    /// pressed key, so Q+W then E fires `Q+W+E`. A key part of a longer combination still fires its own
    /// binding on its own press, so combinations are best built from otherwise free keys.
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

    /// Which command already holds `chord`, besides `command`; the editor names it rather than overwriting.
    pub fn conflict(&self, command: Command, chord: &Chord) -> Option<Command> {
        self.bindings.iter().find(|(k, c)| **k != command && *c == chord).map(|(k, _)| *k)
    }

    /// Assign a binding, refusing on conflict with the holder's name; the caller may retry with `force`.
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
                // Clear the current holder, so the reset cannot create a conflict.
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

    /// Commands sharing a chord; should be empty. For tests and checking imported keymaps.
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
