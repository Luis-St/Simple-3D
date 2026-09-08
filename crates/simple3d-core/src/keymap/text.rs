//! The keymap file: written out and read back.

use super::*;
use std::str::FromStr;

impl Keymap {
    pub fn to_text(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).expect("a keymap always serialises");
        text.push('\n');
        text
    }

    /// Import a keymap file. Anything missing falls back to the recorded
    /// preset's default, so a file from an older build that did not know a
    /// command still yields a fully usable map.
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
            // Never onto the button the user's own orbit already uses: the two
            // would then be the same gesture, and a keymap this build wrote for
            // them is not the place to introduce a conflict.
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
        // Drop commands this build no longer has.
        map.bindings.retain(|k, _| Command::ALL.contains(k));
        Ok(map)
    }
}
