//! Where the configuration directory is.

use super::*;

#[test]
pub(crate) fn the_config_directory_is_absolute_and_named_for_the_app() {
    let dir = config_dir();
    // Spaces removed as well as case: what matters is that the directory is
    // named after the application, not how a platform likes to spell it.
    let text = dir.to_string_lossy().to_lowercase().replace(' ', "");
    assert!(text.contains("simple3d"), "{dir:?}");
}
