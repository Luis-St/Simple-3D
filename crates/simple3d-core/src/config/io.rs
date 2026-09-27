//! Reading and writing the settings and keymap files.

use super::*;
use crate::keymap::Keymap;
use std::path::Path;

/// Load settings, defaulting anything missing or unreadable; a corrupt file must never stop startup.
pub fn load_settings() -> AppSettings {
    load_settings_from(&config_dir())
}

pub fn save_settings(settings: &AppSettings) -> std::io::Result<()> {
    save_settings_to(&config_dir(), settings)
}

/// `load_settings` from an explicit directory, so tests avoid the user's config.
pub fn load_settings_from(dir: &Path) -> AppSettings {
    let mut settings: AppSettings = std::fs::read_to_string(dir.join(SETTINGS_FILE))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    settings.layout.repair();
    settings
}

pub fn save_settings_to(dir: &Path, settings: &AppSettings) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let text = serde_json::to_string_pretty(settings).expect("settings always serialise");
    std::fs::write(dir.join(SETTINGS_FILE), text + "\n")
}

/// The keymap, stored separately so it can be carried between machines (spec section 8.2).
pub fn load_keymap() -> Keymap {
    load_keymap_from(&config_dir())
}

pub fn save_keymap(keymap: &Keymap) -> std::io::Result<()> {
    save_keymap_to(&config_dir(), keymap)
}

/// `load_keymap` from an explicit directory (acceptance criterion 28); unreadable gives the default.
pub fn load_keymap_from(dir: &Path) -> Keymap {
    std::fs::read_to_string(dir.join(KEYMAP_FILE))
        .ok()
        .and_then(|text| Keymap::from_text(&text).ok())
        .unwrap_or_default()
}

pub fn save_keymap_to(dir: &Path, keymap: &Keymap) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join(KEYMAP_FILE), keymap.to_text())
}
