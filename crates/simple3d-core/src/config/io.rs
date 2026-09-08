//! Reading and writing the settings and keymap files.

use super::*;
use crate::keymap::Keymap;
use std::path::Path;

/// Load settings, falling back to defaults for anything missing or unreadable --
/// a corrupt settings file must never stop the application starting.
pub fn load_settings() -> AppSettings {
    load_settings_from(&config_dir())
}

pub fn save_settings(settings: &AppSettings) -> std::io::Result<()> {
    save_settings_to(&config_dir(), settings)
}

/// `load_settings` against an explicit directory, so a test can drive the real
/// startup path against a temp directory instead of the user's own config.
pub fn load_settings_from(dir: &Path) -> AppSettings {
    let mut settings: AppSettings = std::fs::read_to_string(dir.join(SETTINGS_FILE))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    settings.layout.repair();
    settings
}

/// `save_settings` against an explicit directory.
pub fn save_settings_to(dir: &Path, settings: &AppSettings) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let text = serde_json::to_string_pretty(settings).expect("settings always serialise");
    std::fs::write(dir.join(SETTINGS_FILE), text + "\n")
}

/// The keymap is stored separately so it can be exported and imported as one
/// file that a user carries between machines (spec section 8.2).
pub fn load_keymap() -> Keymap {
    load_keymap_from(&config_dir())
}

pub fn save_keymap(keymap: &Keymap) -> std::io::Result<()> {
    save_keymap_to(&config_dir(), keymap)
}

/// `load_keymap` against an explicit directory. The startup path goes through
/// here so a test can drive it against a temp directory rather than the user's
/// real config (acceptance criterion 28). A missing or unreadable file gives the
/// default keymap: a corrupt one must never stop the application starting.
pub fn load_keymap_from(dir: &Path) -> Keymap {
    std::fs::read_to_string(dir.join(KEYMAP_FILE))
        .ok()
        .and_then(|text| Keymap::from_text(&text).ok())
        .unwrap_or_default()
}

/// `save_keymap` against an explicit directory.
pub fn save_keymap_to(dir: &Path, keymap: &Keymap) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join(KEYMAP_FILE), keymap.to_text())
}
