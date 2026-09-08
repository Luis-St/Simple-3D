//! A cancelled or failed export leaves what was already there alone.

use super::*;

#[test]
pub(crate) fn a_cancelled_export_leaves_no_file_behind() {
    // Spec acceptance criterion 16.
    let path = temp_dir().join("cancelled.3mf");
    let mut calls = 0;
    let mut cb = |_: f32| {
        calls += 1;
        calls < 3
    };
    let err = write(&path, &plate(), &Options::default(), &mut cb).unwrap_err();
    assert_eq!(err, ExportError::Cancelled);
    assert!(!path.exists(), "cancelling left a file behind");
    // And nothing half-written next to it either.
    let leftovers: Vec<_> = std::fs::read_dir(temp_dir())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".part"))
        .collect();
    assert!(leftovers.is_empty(), "temporary files left behind: {leftovers:?}");
}

#[test]
pub(crate) fn an_existing_file_survives_a_failed_export() {
    let path = temp_dir().join("existing.stl");
    std::fs::write(&path, b"original contents").unwrap();
    let mut open = plate();
    open.indices.pop();
    let mut cb = no_progress();
    let _ = write(&path, &open, &Options { format: Format::StlBinary, ..Default::default() }, &mut cb);
    assert_eq!(std::fs::read(&path).unwrap(), b"original contents");
    std::fs::remove_file(&path).unwrap();
}
