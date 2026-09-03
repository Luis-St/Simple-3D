//! Saved pattern kinds (issue 67): a custom rule, named and kept for reuse in
//! any project.
//!
//! An entry is one file in `pattern-kinds/` under the config directory, holding
//! the parameters a custom rule is made of and nothing else -- so a saved kind
//! is readable, diffable, and can be handed to someone else by sending them the
//! file, exactly as a saved primitive can (see [`crate::library`]).
//!
//! The library is *per user*, not per project. That is deliberate and it is why
//! a pattern node stores the numbers themselves rather than the name of a kind:
//! a project opened on a machine that has never seen the kind still lays its
//! copies out correctly, because everything the rule needs travels with it.

use crate::pattern;
use crate::primitive::Params;
use std::io;
use std::path::{Path, PathBuf};

const DIRECTORY: &str = "pattern-kinds";
const EXTENSION: &str = "json";

/// One saved kind: what to call it, and where it lives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
}

pub fn dir(config_dir: &Path) -> PathBuf {
    config_dir.join(DIRECTORY)
}

/// Every saved kind, by name. Anything unreadable is skipped rather than
/// reported: a stray file in the directory must not stop the picker drawing.
pub fn list(config_dir: &Path) -> Vec<Entry> {
    let Ok(entries) = std::fs::read_dir(dir(config_dir)) else { return Vec::new() };
    let mut out: Vec<Entry> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == EXTENSION))
        .filter_map(|path| {
            let name = path.file_stem()?.to_string_lossy().to_string();
            Some(Entry { name, path })
        })
        .collect();
    out.sort_by_key(|e| e.name.to_lowercase());
    out
}

pub fn exists(config_dir: &Path, name: &str) -> bool {
    path_for(config_dir, name).exists()
}

/// Write the custom part of `params` to the library under `name`, replacing any
/// entry of that name.
///
/// Only the custom keys are kept. A saved kind is a rule, not a pattern: the
/// linear step and the helix radius that happen to be sitting in the same map
/// are the node's business, and writing them here would mean applying a kind
/// silently changed the numbers of every *other* kind the node could be set to.
pub fn save(config_dir: &Path, name: &str, params: &Params) -> io::Result<PathBuf> {
    let name = crate::library::sanitise(name);
    if name.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "a saved pattern kind needs a name"));
    }
    std::fs::create_dir_all(dir(config_dir))?;
    let path = path_for(config_dir, &name);
    let text = serde_json::to_string_pretty(&extract(params))
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    std::fs::write(&path, text + "\n")?;
    Ok(path)
}

/// The custom rule a file holds, with anything it is missing filled in from the
/// defaults -- so a kind saved by an older build still applies cleanly.
pub fn load(path: &Path) -> Option<Params> {
    let text = std::fs::read_to_string(path).ok()?;
    let stored: Params = serde_json::from_str(&text).ok()?;
    let defaults = pattern::default_params();
    let mut out = Params::new();
    for key in pattern::custom_keys() {
        let value = stored
            .get(key)
            .copied()
            .filter(|v| defaults.get(key).is_some_and(|d| std::mem::discriminant(v) == std::mem::discriminant(d)))
            .or_else(|| defaults.get(key).copied())?;
        out.insert(key.to_string(), value);
    }
    Some(out)
}

pub fn remove(path: &Path) -> io::Result<()> {
    std::fs::remove_file(path)
}

/// Just the parameters that make up a custom rule.
pub fn extract(params: &Params) -> Params {
    pattern::custom_keys().into_iter().filter_map(|key| params.get(key).map(|v| (key.to_string(), *v))).collect()
}

/// Apply a saved rule to a node's parameters: its stages, and the kind choice
/// that makes them the ones in use.
pub fn apply(params: &mut Params, kind: &Params) {
    for (key, value) in kind {
        params.insert(key.clone(), *value);
    }
    params.insert("kind".to_string(), crate::primitive::ParamValue::Choice(pattern::CUSTOM));
}

fn path_for(config_dir: &Path, name: &str) -> PathBuf {
    dir(config_dir).join(format!("{}.{EXTENSION}", crate::library::sanitise(name)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitive::{ParamValue, ParamsExt};

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("simple3d-kinds-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_saved_kind_round_trips_through_a_file() {
        let config = temp_dir("roundtrip");
        let mut params = pattern::default_params();
        params.insert("stages".to_string(), ParamValue::Count(2));
        params.insert("stage2_turn".to_string(), ParamValue::Angle(45.0));
        // A number belonging to another kind entirely, which must not travel.
        params.insert("helix_radius".to_string(), ParamValue::Length(123.0));

        let path = save(&config, "Bolt ring", &params).expect("the kind should save");
        assert_eq!(list(&config).iter().map(|e| e.name.clone()).collect::<Vec<_>>(), vec!["Bolt ring"]);
        assert!(exists(&config, "Bolt ring"));

        let back = load(&path).expect("the kind should load");
        assert_eq!(back.int("stages"), 2);
        assert_eq!(back.num("stage2_turn"), 45.0);
        assert!(!back.contains_key("helix_radius"), "a saved kind carried a number that is not its own");

        // Applying it writes the stages and switches the node to them, without
        // touching what the other kinds hold.
        let mut fresh = pattern::default_params();
        fresh.insert("helix_radius".to_string(), ParamValue::Length(7.0));
        apply(&mut fresh, &back);
        assert_eq!(fresh.int("kind"), pattern::CUSTOM);
        assert_eq!(fresh.int("stages"), 2);
        assert_eq!(fresh.num("helix_radius"), 7.0, "applying a kind rewrote another kind's numbers");

        remove(&path).unwrap();
        assert!(list(&config).is_empty());
        let _ = std::fs::remove_dir_all(&config);
    }

    #[test]
    fn a_kind_file_missing_a_stage_is_filled_in_rather_than_refused() {
        let config = temp_dir("partial");
        std::fs::create_dir_all(dir(&config)).unwrap();
        std::fs::write(dir(&config).join("Half.json"), "{\"stages\":{\"kind\":\"count\",\"value\":2}}").unwrap();
        let back = load(&dir(&config).join("Half.json")).expect("a partial kind should still load");
        assert_eq!(back.int("stages"), 2);
        assert_eq!(back.int("stage1_count"), pattern::default_params().int("stage1_count"));
        let _ = std::fs::remove_dir_all(&config);
    }

    #[test]
    fn a_name_that_is_not_a_file_name_is_refused_rather_than_written_elsewhere() {
        let config = temp_dir("badname");
        assert!(save(&config, "   ", &pattern::default_params()).is_err());
        let path = save(&config, "a/b", &pattern::default_params()).unwrap();
        assert_eq!(path.parent(), Some(dir(&config).as_path()));
        let _ = std::fs::remove_dir_all(&config);
    }
}
