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
///
/// The scatter is kept too when `with_noise` says so (issue 79). A rule for
/// laying planks is a rule *and* the bit of randomness that stops the deck
/// reading as wallpaper, and a kind that came back off the shelf without the
/// second half was only half of what was saved.
pub fn save(config_dir: &Path, name: &str, params: &Params, with_noise: bool) -> io::Result<PathBuf> {
    let name = crate::library::sanitise(name);
    if name.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "a saved pattern kind needs a name"));
    }
    std::fs::create_dir_all(dir(config_dir))?;
    let path = path_for(config_dir, &name);
    let text = serde_json::to_string_pretty(&extract(params, with_noise))
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
    // A rule kept before a stage said what it *does*, or before it could vary
    // its copies more than one way, has to go on laying its copies down where
    // it always did, which is what the old numbers are read for (issue 79).
    pattern::migrate_stages(&stored, &mut out);
    pattern::migrate_noise(&stored, &mut out);
    // The scatter only where the file has one. A kind saved without it leaves
    // whatever scatter the pattern it is applied to already has, rather than
    // taking it off: "no noise was kept" is not "keep no noise".
    for key in pattern::noise_keys() {
        let kept = stored
            .get(*key)
            .copied()
            .filter(|v| defaults.get(*key).is_some_and(|d| std::mem::discriminant(v) == std::mem::discriminant(d)));
        if let Some(value) = kept {
            out.insert((*key).to_string(), value);
        }
    }
    Some(out)
}

pub fn remove(path: &Path) -> io::Result<()> {
    std::fs::remove_file(path)
}

/// Just the parameters that make up a custom rule, and its scatter when asked.
pub fn extract(params: &Params, with_noise: bool) -> Params {
    let noise: &[&str] = if with_noise { pattern::noise_keys() } else { &[] };
    pattern::rule_keys(params)
        .into_iter()
        .chain(noise.iter().map(|key| key.to_string()))
        .filter_map(|key| params.get(&key).copied().map(|v| (key, v)))
        .collect()
}

/// Whether a saved kind carries a scatter of its own.
pub fn has_noise(kind: &Params) -> bool {
    pattern::noise_keys().iter().any(|key| kind.contains_key(*key))
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

        let path = save(&config, "Bolt ring", &params, false).expect("the kind should save");
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

    /// A kind saved with its scatter brings the scatter back and puts it on the
    /// pattern it is applied to; one saved without leaves the pattern's own
    /// alone (issue 79).
    #[test]
    fn a_kind_carries_its_noise_only_when_it_was_saved_with_it() {
        let config = temp_dir("noise");
        let mut params = pattern::default_params();
        params.insert("noise_x".to_string(), ParamValue::Length(1.5));
        params.insert("noise_seed".to_string(), ParamValue::Count(7));

        let with = load(&save(&config, "Planks", &params, true).unwrap()).expect("the kind should load");
        assert!(has_noise(&with));
        assert_eq!(with.num("noise_x"), 1.5);
        assert_eq!(with.int("noise_seed"), 7);
        let mut target = pattern::default_params();
        apply(&mut target, &with);
        assert_eq!(target.num("noise_x"), 1.5, "applying the kind did not bring its scatter");

        let without = load(&save(&config, "Bare", &params, false).unwrap()).expect("the kind should load");
        assert!(!has_noise(&without), "a kind saved without its noise carried it anyway");
        let mut target = pattern::default_params();
        target.insert("noise_y".to_string(), ParamValue::Length(3.0));
        apply(&mut target, &without);
        assert_eq!(target.num("noise_y"), 3.0, "a kind with no noise took the pattern's own off it");
        let _ = std::fs::remove_dir_all(&config);
    }

    /// A kind kept while a stage carried one of each variation comes off the
    /// shelf with that one as a variation of its own, staggering its rows the
    /// way it did when it was saved (issue 79).
    #[test]
    fn a_kind_saved_before_a_stage_held_a_list_keeps_its_stagger() {
        let config = temp_dir("legacy-vary");
        std::fs::create_dir_all(dir(&config)).unwrap();
        let mut old = Params::new();
        old.insert("stages".to_string(), ParamValue::Count(2));
        old.insert("stage2_mode".to_string(), ParamValue::Choice(0));
        old.insert("stage2_shift_x".to_string(), ParamValue::Length(7.0));
        old.insert("stage2_shift_every".to_string(), ParamValue::Count(2));
        std::fs::write(dir(&config).join("Old.json"), serde_json::to_string(&old).unwrap()).unwrap();
        let back = load(&dir(&config).join("Old.json")).expect("an older kind should still load");
        assert_eq!(
            pattern::stage(&back, 1).variations(),
            [pattern::Variation::shift(0, 7.0).repeating(2)],
            "the older kind lost its stagger"
        );
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
        assert!(save(&config, "   ", &pattern::default_params(), false).is_err());
        let path = save(&config, "a/b", &pattern::default_params(), false).unwrap();
        assert_eq!(path.parent(), Some(dir(&config).as_path()));
        let _ = std::fs::remove_dir_all(&config);
    }
}
