//! Bindings that have been retired or moved, so an old keymap file still
//! reads as what it meant.

use super::*;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer};
use std::collections::BTreeMap;

/// Command names earlier builds wrote and this one no longer has.
///
/// A keymap naming one still loads: that binding is dropped and the rest of the
/// user's map survives, because a command being retired is our doing and must
/// not cost them the map they carry between machines. A name that was never a
/// command of ours stays a hard error -- that file is not one this build can
/// honour, and quietly loading half of it is the worse answer.
pub(crate) const RETIRED: [&str; 3] = ["toggle_projection", "toggle_ghosts", "break_apart"];

/// Bindings an older build wrote as *its* default, which this build has moved.
///
/// A keymap file is written back on any change, so every map saved by an older
/// build carries that build's whole default set whether or not the user ever
/// chose any of it. Left alone, a changed default would reach only people with
/// no keymap file -- which is nobody who has used the program. So a binding that
/// is still exactly the old default is moved to the new one, and a binding the
/// user has since changed is left alone, because that one they did choose.
///
/// Snap-to-geometry moved from V to Ctrl on its own when a modifier became
/// bindable at all (issue 77).
pub(crate) const MOVED_DEFAULTS: [(Command, &str, &str); 1] = [(Command::SnapToGeometry, "V", "Ctrl")];

/// The same rule for a navigation drag, which `MOVED_DEFAULTS` cannot carry
/// because a drag is not a `Chord` and pan is not a `Command`.
///
/// `nav` is written back whole on any change, exactly as the bindings are, so a
/// changed navigation default would otherwise reach nobody who has ever used the
/// program. A pan binding that is still precisely the preset's old default is
/// moved to its new one; one the user has since chosen for themselves is theirs.
///
/// Pan moved from Shift+right-drag to the middle button on its own (issue 72).
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
