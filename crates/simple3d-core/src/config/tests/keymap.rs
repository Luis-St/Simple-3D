//! The keymap file beside the settings.

use super::*;
use crate::keymap::Keymap;

/// Spec acceptance criterion 28: a rebinding persists across restart and the menus show it, through
/// the real `save_keymap`/`load_keymap` path in a temp directory.
#[test]
pub(crate) fn a_rebinding_survives_a_restart_and_the_menus_follow() {
    use crate::keymap::{Chord, Command, Preset};

    let dir = temp_dir("restart");
    assert_eq!(load_keymap_from(&dir), Keymap::default(), "an empty config dir must give the default keymap");

    // Switch preset and rebind a command.
    let mut keymap = Keymap::from_preset(Preset::MeshEditor);
    let default_group = Keymap::default().binding(Command::Group).cloned();
    keymap.set(Command::Group, Chord::ctrl_shift("J"), true).unwrap();
    let expected_text = keymap.shortcut_text(Command::Group);
    assert_ne!(Some(&Chord::ctrl_shift("J")), default_group.as_ref(), "the test's chord is already the default");
    save_keymap_to(&dir, &keymap).unwrap();

    // Restart: a fresh read of the same directory.
    let reloaded = load_keymap_from(&dir);
    assert_eq!(reloaded, keymap, "the keymap did not survive the round trip through its file");
    assert_eq!(reloaded.binding(Command::Group), Some(&Chord::ctrl_shift("J")));
    assert_eq!(reloaded.command_for(&Chord::ctrl_shift("J")), Some(Command::Group));
    assert_eq!(reloaded.preset, Preset::MeshEditor, "the preset did not persist");

    // The menus show the reloaded binding.
    assert_eq!(reloaded.shortcut_text(Command::Group), expected_text);
    assert_ne!(
        reloaded.shortcut_text(Command::Group),
        Keymap::default().shortcut_text(Command::Group),
        "the menus would still show the default binding"
    );

    // The file is what a restart reads.
    assert!(dir.join(KEYMAP_FILE).exists());
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A corrupted keymap file starts the application on defaults.
#[test]
pub(crate) fn an_unreadable_keymap_file_falls_back_to_the_default() {
    let dir = temp_dir("corrupt");
    for text in ["", "{", "not json at all", "{\"preset\":\"holographic\"}"] {
        std::fs::write(dir.join(KEYMAP_FILE), text).unwrap();
        assert_eq!(load_keymap_from(&dir), Keymap::default(), "{text:?}");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}
