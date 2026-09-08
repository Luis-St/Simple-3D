//! A key with its modifiers.

/// A key plus modifiers. Serialised as the text the interface shows -- `Ctrl+S`,
/// `Shift+ArrowUp` -- so an exported keymap is readable and hand-editable.
///
/// `keys` may hold any number of them, including none.
///
/// None at all makes the chord the modifiers themselves: `Ctrl` on its own is a
/// binding (issue 77). A modifier is a key like any other -- the one thing a
/// hold-to-snap binding actually wants -- so refusing to store one only meant
/// nobody could bind what they were already reaching for.
///
/// Several makes it a combination of ordinary keys: `Q+W+E` is a binding
/// too, and no key is only ever a *base* for one. What makes both work is the
/// same rule, and it is the one a hand already performs: whatever is held down
/// together is the chord, and it is complete when the hand comes off it.
///
/// The keys are kept sorted, so a chord is the *set* that was held and `Q+W`
/// cannot be bound separately from `W+Q` -- one press cannot mean two things.
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

    /// A combination of ordinary keys, in any order: the set is what counts.
    pub fn combo<I: IntoIterator<Item = S>, S: AsRef<str>>(keys: I) -> Chord {
        let mut chord = Chord {
            keys: keys.into_iter().map(|k| k.as_ref().to_string()).collect(),
            ..Chord::modifiers(false, false, false)
        };
        chord.normalise();
        chord
    }

    /// A chord that is nothing but modifiers, such as the default hold for
    /// geometry snapping.
    pub fn modifiers(ctrl: bool, shift: bool, alt: bool) -> Chord {
        Chord { keys: Vec::new(), ctrl, shift, alt }
    }

    /// Whether this chord is modifiers alone. Such a chord can never be produced
    /// by a key event, so everything that resolves a key press has to skip it,
    /// and everything that reads a held state has to accept it.
    pub fn is_modifier_only(&self) -> bool {
        self.keys.is_empty()
    }

    /// Sorted and deduplicated, which is what makes a chord a set.
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

    /// Whether every key of this chord is in `held`, and its modifiers are
    /// exactly the ones down. What both a hold ("is the snap key down?") and a
    /// press ("did this complete a combination?") ask.
    pub fn satisfied_by(&self, held: impl Fn(&str) -> bool, ctrl: bool, shift: bool, alt: bool) -> bool {
        self.ctrl == ctrl && self.shift == shift && self.alt == alt && self.keys.iter().all(|k| held(k))
    }
}
