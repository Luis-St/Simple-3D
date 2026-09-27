pub mod clipboard;
pub mod config;
pub mod eval;
pub mod keymap;
pub mod library;
pub mod mesh_data;
mod named_files;
pub mod pattern;
pub mod pattern_library;
pub mod primitive;
pub mod project;
pub mod scene;
pub mod undo;
pub mod unit;
pub mod xform;

/// A fresh directory for one test, named by `tag`, the process and the thread, so parallel tests
/// never share one.
#[cfg(test)]
pub(crate) fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "simple3d-core-test-{tag}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
