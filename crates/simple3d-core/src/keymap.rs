//! Remappable keyboard and mouse bindings (spec section 8.2).
//!
//! *Every* command and *every* navigation binding is remappable, not a chosen
//! subset -- including the mouse buttons and modifiers for orbit and pan and the
//! wheel direction for zoom. Ships with selectable presets so someone arriving
//! from another program is productive immediately, and a preset is a starting
//! point the user can then modify.
//!
//! Key names are the strings the UI toolkit uses for its own key enum ("A",
//! "Up", "Escape", "F2"), so no translation table can drift out of date. There
//! is a test in the app crate that checks every preset binding against the
//! toolkit's actual key list, so a binding nobody can type cannot ship.
//! A keymap saved on one platform therefore loads sensibly on the other.

mod command;
pub use command::{Area, Command};
mod chord;
mod command_label;
mod command_list;
pub use chord::Chord;
mod chord_text;
mod mouse;
pub use mouse::{Drag, MouseButton};
mod nav;
pub use nav::{NavMap, Preset};
mod migrate;
mod preset;
pub(crate) use migrate::*;
mod lookup;
#[cfg(test)]
mod tests;
mod text;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keymap {
    pub preset: Preset,
    #[serde(deserialize_with = "bindings_from_names")]
    pub bindings: BTreeMap<Command, Chord>,
    pub nav: NavMap,
}

impl Default for Keymap {
    fn default() -> Self {
        Keymap::from_preset(Preset::Default)
    }
}
