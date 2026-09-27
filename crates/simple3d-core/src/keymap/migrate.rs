//! Retired or moved bindings, so old keymap files still read as meant.

use super::*;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer};
use std::collections::BTreeMap;

/// Command names earlier builds wrote that no longer exist. Those bindings are dropped and the rest
/// load; an unknown name that was never ours stays a hard error.
pub(crate) const RETIRED: [&str; 4] = ["toggle_projection", "toggle_ghosts", "break_apart", "toggle_handle_frame"];

/// Old defaults this build has moved. Keymap files carry the whole default set, so a binding still
/// exactly at the old default is moved; one the user changed is kept. Snap moved from V to Ctrl
/// alone (issue 77).
pub(crate) const MOVED_DEFAULTS: [(Command, &str, &str); 1] = [(Command::SnapToGeometry, "V", "Ctrl")];

/// The same rule for the pan drag, which is not a `Chord` or `Command`. Pan moved from
/// Shift+right-drag to the middle button (issue 72).
pub(crate) fn moved_nav_pan(preset: Preset) -> Option<(Drag, Drag)> {
    match preset {
        Preset::Default => Some((Drag::with_shift(MouseButton::Right), Drag::new(MouseButton::Middle))),
        Preset::MeshEditor | Preset::Cad => None,
    }
}

pub(crate) fn bindings_from_names<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<Command, Chord>, D::Error> {
    use serde::de::IntoDeserializer;
    let named: BTreeMap<String, Chord> = BTreeMap::deserialize(deserializer)?;
    let mut bindings = BTreeMap::new();
    for (name, chord) in named {
        if RETIRED.contains(&name.as_str()) {
            continue;
        }
        let command: Command =
            Command::deserialize(IntoDeserializer::<serde::de::value::Error>::into_deserializer(name.as_str()))
                .map_err(|_| D::Error::custom(format!("unknown command \"{name}\"")))?;
        bindings.insert(command, chord);
    }
    Ok(bindings)
}
