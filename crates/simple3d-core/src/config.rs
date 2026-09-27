//! Per-user settings outside any project (spec sections 7.1, 9, 11): window, panels, display mode,
//! keymap, recent files, last export choices. Stored per-user or beside the executable in portable
//! mode; missing or unreadable settings fall back to defaults.

mod display;
pub use display::{DisplayMode, RenderEngine};
mod snap;
pub use snap::SnapMode;
mod open;
pub use open::OpenTarget;
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

/// One row of swatches; more would be a list to search.
const MAX_RECENT_COLOURS: usize = 8;
