//! Per-user settings that are *not* part of a project (spec sections 7.1, 9,
//! 11): window geometry, panel sizes, display mode, the keymap, the recent-file
//! list and the last export choices.
//!
//! Stored in the platform-appropriate per-user location, with a portable mode
//! that keeps everything beside the executable instead. The binary never
//! requires the source tree, a working directory or sibling files to be present:
//! if the settings file is missing or unreadable, defaults apply.

mod display;
pub use display::{DisplayMode, RenderEngine};
mod snap;
pub use snap::SnapMode;
mod layout;
pub use layout::{Layout, Panel, Placement, Side};
mod settings;
pub use settings::{indistinguishable, AppSettings};
mod paths;
pub use paths::{config_dir, portable_mode};
mod io;
pub use io::{
    load_keymap, load_keymap_from, load_settings, load_settings_from, save_keymap, save_keymap_to, save_settings,
    save_settings_to,
};
#[cfg(test)]
mod tests;

const SETTINGS_FILE: &str = "settings.json";

const KEYMAP_FILE: &str = "keymap.json";

const MAX_RECENT: usize = 10;

/// A row of swatches, no more: past that it is a list to search rather than a
/// set to glance at.
const MAX_RECENT_COLOURS: usize = 8;
