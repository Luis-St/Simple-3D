//! Where the configuration directory is.

use super::*;

#[test]
pub(crate) fn the_config_directory_is_absolute_and_named_for_the_app() {
    let dir = config_dir();
    // Case and spaces ignored: only that it is named after the application matters.
    let text = dir.to_string_lossy().to_lowercase().replace(' ', "");
    assert!(text.contains("simple3d"), "{dir:?}");
}
