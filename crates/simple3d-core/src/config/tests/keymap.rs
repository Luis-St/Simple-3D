//! The keymap file beside the settings.

use super::*;
use crate::keymap::Keymap;

/// Spec acceptance criterion 28: restart after rebinding and the keymap has
/// persisted, with the menus showing the new binding rather than the default.
///
/// This drives the real startup path -- `save_keymap` then `load_keymap` over
/// a directory -- rather than the serialised form alone, because that glue is
/// the whole of what "persisted across a restart" means. The directory is a
/// temp one so the test cannot touch the user's own config.
#[test]
pub(crate) fn a_rebinding_survives_a_restart_and_the_menus_follow() {
    use crate::keymap::{Chord, Command, Preset};

    let dir = temp_dir("restart");
    assert_eq!(load_keymap_from(&dir), Keymap::default(), "an empty config dir must give the default keymap");

    // Switch preset and rebind a command onto a combination of our choosing.
    let mut keymap = Keymap::from_preset(Preset::MeshEditor);
    let default_group = Keymap::default().binding(Command::Group).cloned();
    keymap.set(Command::Group, Chord::ctrl_shift("J"), true).unwrap();
    let expected_text = keymap.shortcut_text(Command::Group);
    assert_ne!(Some(&Chord::ctrl_shift("J")), default_group.as_ref(), "the test's chord is already the default");
    save_keymap_to(&dir, &keymap).unwrap();

    // Restart: a fresh process reads the same directory and knows nothing
    // else about the session that wrote it.
    let reloaded = load_keymap_from(&dir);
    assert_eq!(reloaded, keymap, "the keymap did not survive the round trip through its file");
    assert_eq!(reloaded.binding(Command::Group), Some(&Chord::ctrl_shift("J")));
    assert_eq!(reloaded.command_for(&Chord::ctrl_shift("J")), Some(Command::Group));
    assert_eq!(reloaded.preset, Preset::MeshEditor, "the preset did not persist");

    // What the menus and tooltips render is the reloaded binding, not the
    // default one they would show from a fresh `Keymap`.
    assert_eq!(reloaded.shortcut_text(Command::Group), expected_text);
    assert_ne!(
        reloaded.shortcut_text(Command::Group),
        Keymap::default().shortcut_text(Command::Group),
        "the menus would still show the default binding"
    );

    // And the file really is what a restart reads: nothing lingers in memory.
    assert!(dir.join(KEYMAP_FILE).exists());
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A keymap file that has been corrupted must leave the application startable
/// on the defaults rather than failing to launch.
#[test]
pub(crate) fn an_unreadable_keymap_file_falls_back_to_the_default() {
    let dir = temp_dir("corrupt");
    for text in ["", "{", "not json at all", "{\"preset\":\"holographic\"}"] {
        std::fs::write(dir.join(KEYMAP_FILE), text).unwrap();
        assert_eq!(load_keymap_from(&dir), Keymap::default(), "{text:?}");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}
