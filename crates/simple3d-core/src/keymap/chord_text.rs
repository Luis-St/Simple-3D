//! A chord as text: shown, stored and parsed.

use super::*;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Joined, so a modifier-only chord reads `Ctrl`, not `Ctrl+`, and still round-trips.
        let mut parts: Vec<&str> = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }
        parts.extend(self.keys.iter().map(String::as_str));
        write!(f, "{}", parts.join("+"))
    }
}

impl FromStr for Chord {
    type Err = String;

    fn from_str(text: &str) -> Result<Chord, String> {
        let mut chord = Chord::modifiers(false, false, false);
        for part in text.split('+') {
            match part.trim() {
                "" => return Err(format!("empty key in binding {text:?}")),
                "Ctrl" | "Control" | "Cmd" | "Command" => chord.ctrl = true,
                "Alt" | "Option" => chord.alt = true,
                "Shift" => chord.shift = true,
                key => chord.keys.push(key.to_string()),
            }
        }
        // Modifiers alone are a binding (issue 77); an empty chord is not.
        if chord.keys.is_empty() && !(chord.ctrl || chord.shift || chord.alt) {
            return Err(format!("no key in binding {text:?}"));
        }
        chord.normalise();
        Ok(chord)
    }
}

impl Serialize for Chord {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Chord {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Chord, D::Error> {
        let text = String::deserialize(d)?;
        Chord::from_str(&text).map_err(D::Error::custom)
    }
}
