//! The keymap file: written out and read back.

use super::*;
use std::str::FromStr;

impl Keymap {
    pub fn to_text(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).expect("a keymap always serialises");
        text.push('\n');
        text
    }

    /// Import a keymap file; anything missing falls back to the preset, so older files stay usable.
    pub fn from_text(text: &str) -> Result<Keymap, String> {
        let mut map: Keymap = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let preset = Keymap::from_preset(map.preset);
        for (command, was, now) in MOVED_DEFAULTS {
            let (was, now) = (Chord::from_str(was)?, Chord::from_str(now)?);
            if map.bindings.get(&command) == Some(&was) && map.conflict(command, &now).is_none() {
                map.bindings.insert(command, now);
            }
        }
        if let Some((was, now)) = moved_nav_pan(map.preset) {
            // Never onto the button the user's orbit uses, which would create a conflict.
            if map.nav.pan == was && !map.nav.orbit.matches(now.button, now.ctrl, now.shift, now.alt) {
                map.nav.pan = now;
            }
        }
        for command in Command::ALL {
            if !map.bindings.contains_key(command) {
                if let Some(chord) = preset.bindings.get(command) {
                    if map.conflict(*command, chord).is_none() {
                        map.bindings.insert(*command, chord.clone());
                    }
                }
            }
        }
        map.bindings.retain(|k, _| Command::ALL.contains(k));
        Ok(map)
    }
}
