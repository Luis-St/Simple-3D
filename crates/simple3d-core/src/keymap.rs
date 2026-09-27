//! Remappable keyboard and mouse bindings (spec section 8.2).
//!
//! Every command and navigation binding is remappable, with presets as starting points. Key names
//! are the UI toolkit's own ("A", "Up", "F2"), so no translation table can drift; an app-crate
//! test checks every preset binding against the toolkit's key list.

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
