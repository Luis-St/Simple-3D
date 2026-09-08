//! Keeping a name unique in the tree, and what a copy of one is called.

use std::collections::HashSet;

/// What a copy of `name` is called before it is numbered: `Box` -> `Box copy`,
/// and `Box copy` -> `Box copy` again rather than `Box copy copy`.
///
/// Duplicating repeatedly is the ordinary way to lay out a row of something, and
/// each duplicate is of the one just made -- so without the trim the fourth
/// press of Ctrl+D gives "Box copy copy copy copy". Trimmed, the numbering rule
/// below takes over and gives "Box copy 2", "Box copy 3".
pub fn copy_name(name: &str) -> String {
    let stem = name.trim_end_matches(|c: char| c.is_ascii_digit()).trim_end();
    let stem = stem.strip_suffix(" copy").unwrap_or(name);
    format!("{stem} copy")
}

/// `base`, or `base 2`, `base 3`... -- the first that is not in `taken`.
///
/// The one place the numbering rule lives, so a node added, pasted, duplicated
/// or dropped in from the library all read the same way in the outliner.
pub fn free_name(taken: &HashSet<String>, base: &str) -> String {
    if !taken.contains(base) {
        return base.to_string();
    }
    for n in 2.. {
        let candidate = format!("{base} {n}");
        if !taken.contains(&candidate) {
            return candidate;
        }
    }
    unreachable!()
}
