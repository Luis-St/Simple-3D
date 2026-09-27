//! Keeping a name unique in the tree, and what a copy is called.

use std::collections::HashSet;

/// A copy's base name: `Box` -> `Box copy`, and `Box copy` stays `Box copy`, so repeated duplicates
/// number up ("Box copy 2") instead of stacking "copy".
pub fn copy_name(name: &str) -> String {
    let stem = name.trim_end_matches(|c: char| c.is_ascii_digit()).trim_end();
    let stem = stem.strip_suffix(" copy").unwrap_or(name);
    format!("{stem} copy")
}

/// Whether `name` was given by the numbering rule for `base` (`base`, `base 2`, `base copy`,
/// `base copy 2`) rather than typed.
pub fn is_default_name(name: &str, base: &str) -> bool {
    let numbered = |name: &str, stem: &str| {
        name == stem
            || name
                .strip_prefix(stem)
                .and_then(|rest| rest.strip_prefix(' '))
                .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
    };
    numbered(name, base) || numbered(name, &format!("{base} copy"))
}

/// `base`, or `base 2`, `base 3`...: the first not in `taken`. The one place the numbering lives.
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
