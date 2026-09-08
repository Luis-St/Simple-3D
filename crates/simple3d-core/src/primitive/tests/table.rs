//! The table itself: unique ids, unique keys, and migration.

use super::*;

#[test]
pub(crate) fn migration_fills_new_parameters_and_drops_unknown_ones() {
    let spec = lookup("box").unwrap();
    let mut stored: Params = Params::new();
    stored.insert("width".into(), ParamValue::Length(55.0));
    stored.insert("obsolete".into(), ParamValue::Length(1.0));
    let migrated = spec.migrate_params(&stored);
    assert_eq!(migrated.num("width"), 55.0);
    assert_eq!(migrated.num("depth"), 20.0);
    assert!(!migrated.contains_key("obsolete"));
    assert_eq!(migrated.len(), spec.params.len());
}

#[test]
pub(crate) fn parameter_keys_are_unique_within_a_type() {
    for spec in REGISTRY {
        for (i, p) in spec.params.iter().enumerate() {
            assert!(
                !spec.params[..i].iter().any(|q| q.key == p.key),
                "{}: duplicate parameter key {}",
                spec.type_id,
                p.key
            );
        }
        if let Some((dep, _)) = spec.params.iter().find_map(|p| p.shown_when) {
            assert!(spec.param(dep).is_some(), "{}: shown_when names unknown {dep}", spec.type_id);
        }
    }
}

#[test]
pub(crate) fn type_ids_are_unique() {
    for (i, spec) in REGISTRY.iter().enumerate() {
        assert!(REGISTRY[..i].iter().all(|s| s.type_id != spec.type_id), "duplicate {}", spec.type_id);
    }
}
