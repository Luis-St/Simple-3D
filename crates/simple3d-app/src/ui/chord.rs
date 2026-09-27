//! Watching a chord being held down.

use simple3d_core::keymap::Chord;

/// Tracks what is held so a modifier alone (issue 77), a combination like `Q+W+E`, or a mix can be
/// a binding. The toolkit reports modifiers only as state, and a key press cannot be told from a
/// combination's start until release, so the chord is everything held together, complete when the
/// last is released; the widest set held during the press is reported.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChordHold {
    pub(super) held: Option<Chord>,
    /// The pointer was used during the hold, making it part of a mouse gesture, not a binding.
    pub(super) interrupted: bool,
}

impl ChordHold {
    /// Feed one frame; returns a chord if this frame completed one (all released, pointer unused).
    /// `keys_down` are held ordinary keys; `interrupted` is mouse use, so Ctrl+click does not fire Ctrl.
    pub fn update<'a>(
        &mut self,
        modifiers: egui::Modifiers,
        keys_down: impl IntoIterator<Item = &'a str>,
        interrupted: bool,
    ) -> Option<Chord> {
        let mut now = Chord {
            keys: keys_down.into_iter().map(str::to_string).collect(),
            ctrl: modifiers.command,
            shift: modifiers.shift,
            alt: modifiers.alt,
        };
        if now.keys.is_empty() && !(now.ctrl || now.shift || now.alt) {
            let released = self.held.take();
            let clean = !self.interrupted;
            self.interrupted = false;
            return released.filter(|_| clean);
        }
        if let Some(held) = &self.held {
            now.keys.extend(held.keys.iter().cloned());
            now.ctrl |= held.ctrl;
            now.shift |= held.shift;
            now.alt |= held.alt;
        }
        now.keys.sort();
        now.keys.dedup();
        self.held = Some(now);
        self.interrupted |= interrupted;
        None
    }

    /// Forget a hold in progress, when the keyboard has gone elsewhere and the release will not be seen.
    pub fn reset(&mut self) {
        *self = ChordHold::default();
    }
}

/// Every ordinary key held now, by keymap name.
pub fn keys_down(input: &egui::InputState) -> Vec<String> {
    let mut names: Vec<String> = input.keys_down.iter().map(|k| k.name().to_string()).collect();
    names.sort();
    names
}
