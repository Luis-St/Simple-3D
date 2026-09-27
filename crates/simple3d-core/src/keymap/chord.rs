//! A key with its modifiers.

/// A set of keys plus modifiers, serialised as the interface shows it (`Ctrl+S`) so keymaps are
/// hand-editable. No keys makes a modifier-only binding (`Ctrl`, issue 77); several make a
/// combination (`Q+W+E`). Whatever is held together is the chord, complete on release. Keys are
/// sorted, so `Q+W` and `W+Q` are the same chord.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Chord {
    pub keys: Vec<String>,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Chord {
    pub fn key(key: &str) -> Chord {
        Chord { keys: vec![key.to_string()], ctrl: false, shift: false, alt: false }
    }

    /// A combination of ordinary keys; order does not matter.
    pub fn combo<I: IntoIterator<Item = S>, S: AsRef<str>>(keys: I) -> Chord {
        let mut chord = Chord {
            keys: keys.into_iter().map(|k| k.as_ref().to_string()).collect(),
            ..Chord::modifiers(false, false, false)
        };
        chord.normalise();
        chord
    }

    /// A modifier-only chord, such as the default snap hold.
    pub fn modifiers(ctrl: bool, shift: bool, alt: bool) -> Chord {
        Chord { keys: Vec::new(), ctrl, shift, alt }
    }

    /// Whether this chord is modifiers alone: never produced by a key event, so press resolution skips
    /// it and held-state checks accept it.
    pub fn is_modifier_only(&self) -> bool {
        self.keys.is_empty()
    }

    /// Sorted and deduplicated, making the chord a set.
    pub(super) fn normalise(&mut self) {
        self.keys.sort();
        self.keys.dedup();
    }

    pub fn ctrl(key: &str) -> Chord {
        Chord { ctrl: true, ..Chord::key(key) }
    }

    pub fn ctrl_shift(key: &str) -> Chord {
        Chord { ctrl: true, shift: true, ..Chord::key(key) }
    }

    pub fn shift(key: &str) -> Chord {
        Chord { shift: true, ..Chord::key(key) }
    }

    pub fn alt(key: &str) -> Chord {
        Chord { alt: true, ..Chord::key(key) }
    }

    /// Whether every key is in `held` and exactly these modifiers are down, for both holds and presses.
    pub fn satisfied_by(&self, held: impl Fn(&str) -> bool, ctrl: bool, shift: bool, alt: bool) -> bool {
        self.ctrl == ctrl && self.shift == shift && self.alt == alt && self.keys.iter().all(|k| held(k))
    }
}
