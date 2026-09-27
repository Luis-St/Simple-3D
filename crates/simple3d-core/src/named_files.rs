//! A directory holding one JSON file per saved entry, named after it: what the saved-primitive
//! and saved-kind libraries share.

use std::io;
use std::path::{Path, PathBuf};

const EXTENSION: &str = "json";

/// One saved entry: its name and file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
}

/// Every entry in `dir` by name; unreadable files are skipped so the palette still draws.
pub(crate) fn list(dir: &Path) -> Vec<Entry> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<Entry> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == EXTENSION))
        .filter_map(|path| {
            let name = path.file_stem()?.to_string_lossy().to_string();
            Some(Entry { name, path })
        })
        .collect();
    // Case-insensitive, so the list reads as one.
    out.sort_by_key(|e| e.name.to_lowercase());
    out
}

pub(crate) fn path_for(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{}.{EXTENSION}", sanitise(name)))
}

/// The file to write `name` to in `dir`, which is created. A name that is empty once sanitised is
/// refused, with `what` naming the kind of entry.
pub(crate) fn prepare_save(dir: &Path, name: &str, what: &str) -> io::Result<PathBuf> {
    let name = sanitise(name);
    if name.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("a {what} needs a name")));
    }
    std::fs::create_dir_all(dir)?;
    Ok(path_for(dir, &name))
}

pub fn remove(path: &Path) -> io::Result<()> {
    std::fs::remove_file(path)
}

/// A name safe as a file name on every platform, since separators or reserved characters would
/// misplace or lose the entry.
pub fn sanitise(name: &str) -> String {
    let cleaned: String =
        name.chars().map(|c| if c.is_control() || "/\\:*?\"<>|".contains(c) { '-' } else { c }).collect();
    cleaned.trim().trim_matches('.').to_string()
}
