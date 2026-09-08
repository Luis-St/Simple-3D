//! The recent files and recent colours lists.

use super::*;
use std::path::{Path, PathBuf};

#[test]
pub(crate) fn recent_files_are_most_recent_first_deduplicated_and_bounded() {
    let mut settings = AppSettings::default();
    for i in 0..MAX_RECENT + 5 {
        settings.remember_recent(Path::new(&format!("/tmp/p{i}.simple3d")));
    }
    assert_eq!(settings.recent_files.len(), MAX_RECENT);
    assert_eq!(settings.recent_files[0], PathBuf::from("/tmp/p14.simple3d"));

    settings.remember_recent(Path::new("/tmp/p10.simple3d"));
    assert_eq!(settings.recent_files[0], PathBuf::from("/tmp/p10.simple3d"));
    assert_eq!(settings.recent_files.iter().filter(|p| p.ends_with("p10.simple3d")).count(), 1);

    settings.forget_recent(Path::new("/tmp/p10.simple3d"));
    assert!(!settings.recent_files.contains(&PathBuf::from("/tmp/p10.simple3d")));
}

#[test]
pub(crate) fn recent_colours_are_most_recent_first_deduplicated_and_bounded() {
    let mut settings = AppSettings::default();
    // Twenty apart, so each is a colour of its own rather than a shade of
    // the one before it.
    let step = 20_u8;
    for i in 0..MAX_RECENT_COLOURS + 4 {
        settings.remember_colour([i as u8 * step, 0, 0]);
    }
    assert_eq!(settings.recent_colours.len(), MAX_RECENT_COLOURS);
    assert_eq!(settings.recent_colours[0], [(MAX_RECENT_COLOURS + 3) as u8 * step, 0, 0]);

    // Using one again moves it to the front rather than repeating it.
    let again = settings.recent_colours[3];
    settings.remember_colour(again);
    assert_eq!(settings.recent_colours[0], again);
    assert_eq!(settings.recent_colours.iter().filter(|c| **c == again).count(), 1);
}

/// Issue 85: the row holds colours to click, not a record of where a drag
/// went. A shade of one already on it takes that one's slot.
#[test]
pub(crate) fn a_shade_of_a_remembered_colour_takes_its_slot_rather_than_another() {
    let mut settings = AppSettings::default();
    settings.remember_colour([0x30, 0x40, 0x50]);
    settings.remember_colour([0x35, 0x44, 0x4A]);
    assert_eq!(settings.recent_colours, vec![[0x35, 0x44, 0x4A]]);
    // Far enough apart to be another colour.
    settings.remember_colour([0x45, 0x44, 0x4A]);
    assert_eq!(settings.recent_colours, vec![[0x45, 0x44, 0x4A], [0x35, 0x44, 0x4A]]);

    assert!(indistinguishable([0, 0, 0], [9, 9, 9]), "nine of 255 on a channel is the same black");
    assert!(!indistinguishable([0, 0, 0], [0, 0, 11]), "a channel eleven apart is another colour");
}
