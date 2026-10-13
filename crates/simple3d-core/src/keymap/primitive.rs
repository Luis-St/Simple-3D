//! Shortcuts that add a primitive (issue 124), set from the palette rather than the keymap editor.

use super::*;

/// What a chord can be bound to: a command, or adding a primitive by its registry id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Command(Command),
    Primitive(String),
}

impl Action {
    /// How the interface names it, for conflicts and the editor.
    pub fn label(&self) -> String {
        match self {
            Action::Command(command) => command.label().to_string(),
            Action::Primitive(type_id) => {
                format!("Add {}", crate::primitive::lookup(type_id).map_or(type_id.as_str(), |spec| spec.label))
            }
        }
    }
}

impl Keymap {
    pub fn primitive_binding(&self, type_id: &str) -> Option<&Chord> {
        self.primitives.get(type_id)
    }

    /// The label for a primitive's shortcut; empty when it has none.
    pub fn primitive_shortcut_text(&self, type_id: &str) -> String {
        self.primitives.get(type_id).map(|chord| chord.to_string()).unwrap_or_default()
    }

    /// What holds `chord`, besides `except`; the interface names it rather than overwriting.
    pub fn holder(&self, chord: &Chord, except: Option<&Action>) -> Option<Action> {
        let command = self.bindings.iter().find(|(c, held)| *held == chord && except != Some(&Action::Command(**c)));
        let primitive = || {
            self.primitives
                .iter()
                .find(|(id, held)| *held == chord && !matches!(except, Some(Action::Primitive(e)) if e == *id))
                .map(|(id, _)| Action::Primitive(id.clone()))
        };
        command.map(|(c, _)| Action::Command(*c)).or_else(primitive)
    }

    /// Bind a primitive, refusing on conflict with the holder; the caller may retry with `force`.
    pub fn set_primitive(&mut self, type_id: &str, chord: Chord, force: bool) -> Result<(), Action> {
        if let Some(holder) = self.holder(&chord, Some(&Action::Primitive(type_id.to_string()))) {
            if !force {
                return Err(holder);
            }
            self.release(&holder);
        }
        self.primitives.insert(type_id.to_string(), chord);
        Ok(())
    }

    /// [`Keymap::set`] or [`Keymap::set_primitive`], whichever `action` needs.
    pub fn bind(&mut self, action: &Action, chord: Chord, force: bool) -> Result<(), Action> {
        match action {
            Action::Command(command) => self.set(*command, chord, force),
            Action::Primitive(type_id) => self.set_primitive(type_id, chord, force),
        }
    }

    pub fn unbind_primitive(&mut self, type_id: &str) {
        self.primitives.remove(type_id);
    }

    /// Take `action`'s binding away.
    pub(super) fn release(&mut self, action: &Action) {
        match action {
            Action::Command(command) => self.bindings.remove(command),
            Action::Primitive(type_id) => self.primitives.remove(type_id),
        };
    }

    /// What a key press fires, commands and primitives alike; see [`Keymap::command_for_press`].
    pub fn action_for_press(
        &self,
        pressed: &str,
        held: impl Fn(&str) -> bool,
        ctrl: bool,
        shift: bool,
        alt: bool,
    ) -> Option<Action> {
        let commands = self.bindings.iter().map(|(command, chord)| (Action::Command(*command), chord));
        let primitives = self.primitives.iter().map(|(id, chord)| (Action::Primitive(id.clone()), chord));
        commands
            .chain(primitives)
            .filter(|(_, chord)| chord.keys.iter().any(|k| k == pressed))
            .filter(|(_, chord)| chord.satisfied_by(&held, ctrl, shift, alt))
            .max_by_key(|(_, chord)| chord.keys.len())
            .map(|(action, _)| action)
    }

    /// What a completed chord is bound to, for modifier-only holds.
    pub fn action_for(&self, chord: &Chord) -> Option<Action> {
        self.holder(chord, None)
    }
}
