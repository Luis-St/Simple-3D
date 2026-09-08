//! Watching a chord being held down.

use simple3d_core::keymap::Chord;

/// Watches what is being held so that whatever a hand holds down together can be
/// a binding: a modifier on its own (issue 77), an ordinary combination like
/// `Q+W+E`, or the two mixed.
///
/// The toolkit never reports Ctrl, Shift or Alt as key events -- they only ever
/// arrive as the modifier state of some *other* key -- so a modifier press has
/// to be recognised from that state changing. Ordinary keys have the same
/// problem in reverse: a press cannot be told from the start of a combination
/// until the hand comes off. So one rule covers both, and it is the one a hand
/// already performs: everything held down together is the chord, and the chord
/// is complete when the last of it is released.
///
/// The widest set held during one press is what comes out, so pressing Q, adding
/// W and E and letting all three go is `Q+W+E` rather than whichever happened to
/// be released last.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChordHold {
    pub(super) held: Option<Chord>,
    /// The pointer was used while this was down, so the hold is part of a mouse
    /// gesture and not a binding of its own.
    pub(super) interrupted: bool,
}

impl ChordHold {
    /// Feed one frame's state. Returns the chord if this frame completed one:
    /// everything is up again and the pointer was not used along the way.
    ///
    /// `keys_down` is every ordinary key currently held, and `interrupted` is a
    /// mouse button being used -- Ctrl+click picks a second object and must not
    /// also fire what Ctrl alone is bound to.
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

    /// Forget a hold in progress: the keyboard has gone somewhere else -- a
    /// dialog, a text field -- and the release will never be seen here.
    pub fn reset(&mut self) {
        *self = ChordHold::default();
    }
}

/// Every ordinary key held down right now, by the name the keymap stores.
pub fn keys_down(input: &egui::InputState) -> Vec<String> {
    let mut names: Vec<String> = input.keys_down.iter().map(|k| k.name().to_string()).collect();
    names.sort();
    names
}
