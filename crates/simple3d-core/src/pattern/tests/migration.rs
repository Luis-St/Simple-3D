//! Rules saved by older versions keep their copies where they were (issue 79).

use super::*;
use crate::primitive::{ParamValue, ParamsExt};
use simple3d_geom::Vec3;

/// A rule saved with one of each old per-stage field migrates to variations that place the
/// copies unchanged.
#[test]
pub(crate) fn a_stage_from_before_it_held_a_list_keeps_what_it_varied() {
    let mut old = rule(&[Stage::run(4, Vec3::new(10.0, 0.0, 0.0)), Stage::turning(3, 30.0, 20.0, 0.0, 0.0, 2)]);
    for k in &STAGES {
        old.remove(k.variations);
    }
    for (key, value) in [
        ("stage1_gap_growth", ParamValue::Length(2.0)),
        ("stage1_shift_x", ParamValue::Length(5.0)),
        ("stage1_shift_every", ParamValue::Count(3)),
        ("stage1_spin", ParamValue::Angle(10.0)),
        ("stage1_scale", ParamValue::Count(90)),
        // A turn never had gaps, so this must not become a variation.
        ("stage2_gap_growth", ParamValue::Length(4.0)),
        ("stage2_spin", ParamValue::Angle(5.0)),
    ] {
        old.insert(key.to_string(), value);
    }
    let migrated = migrate_params(&old);
    let run = stage(&migrated, 0);
    assert_eq!(
        run.variations(),
        [
            Variation::widen(2.0),
            // A cycle of three gave 0, 1 and 2 steps: two variations, reaching the one-step and two-step copies.
            Variation::shift(0, 5.0).repeating(3).reaching(3, 2),
            Variation::shift(0, 10.0).repeating(3).reaching(3, 3),
            Variation::spin(2, 10.0),
            Variation::resize(ALL_AXES, 0.9),
        ],
        "the old stage's one section did not become its list"
    );
    assert_eq!(stage(&migrated, 1).variations(), [Variation::spin(2, 5.0)], "a turn was given a gap it never had");

    let copy = run.place(2);
    assert!(near(copy.t, Vec3::new(22.0 + 10.0, 0.0, 0.0)), "copy 2 landed at {:?}", copy.t);
    assert!((copy.axis_vector(0).length() - 0.81).abs() < 1e-12);
    let x = copy.axis_vector(0);
    assert!((x.y.atan2(x.x).to_degrees() - 20.0).abs() < 1e-9);
    // Migration is idempotent.
    assert_eq!(migrate_params(&migrated), migrated);
}

/// A rule saved with four variation slots migrates without moving copies and drops the old names.
#[test]
pub(crate) fn a_stage_from_when_it_had_four_slots_keeps_its_copies_where_they_were() {
    let mut old = rule(&[Stage::run(5, Vec3::new(10.0, 0.0, 0.0))]);
    old.remove("stage1_variations");
    for (key, value) in [
        ("stage1_varied", ParamValue::Count(2)),
        ("stage1_vary1_what", ParamValue::Choice(0)),
        ("stage1_vary1_steps", ParamValue::Choice(1)),
        ("stage1_vary1_every", ParamValue::Count(3)),
        ("stage1_vary1_x", ParamValue::Length(1.0)),
        ("stage1_vary1_z", ParamValue::Length(2.0)),
        ("stage1_vary2_what", ParamValue::Choice(3)),
        ("stage1_vary2_steps", ParamValue::Choice(0)),
        ("stage1_vary2_gap", ParamValue::Length(4.0)),
    ] {
        old.insert(key.to_string(), value);
    }
    let migrated = migrate_params(&old);
    assert!(!migrated.keys().any(|key| key.contains("_vary")), "the four slots' old names were kept");
    // Copy i was shifted (i mod 3) steps, and its gaps grew by 4 a copy.
    let copies = instances(&migrated);
    for (i, copy) in copies.iter().enumerate() {
        let cycle = (i % 3) as f64;
        let along = 10.0 * i as f64 + 4.0 * (i * i.saturating_sub(1) / 2) as f64;
        let want = Vec3::new(along + cycle, 0.0, 2.0 * cycle);
        assert!(near(copy.xform.t, want), "copy {i} is at {:?}, not {want:?}", copy.xform.t);
    }
    assert_eq!(migrate_params(&migrated), migrated);
}

/// A rule from before stage modes still lays its copies where it did (issue 79).
#[test]
pub(crate) fn a_rule_from_before_the_stage_modes_lays_out_the_same_copies() {
    // The old helix stage: a turn, a radius, and the rise as a step along the stage's axis.
    let mut old = default_params();
    for key in custom_keys() {
        if key.ends_with("_mode") || key.ends_with("_rise") {
            old.remove(key);
        }
    }
    old.insert("kind".to_string(), ParamValue::Choice(CUSTOM));
    old.insert("stage1_turn".to_string(), ParamValue::Angle(45.0));
    old.insert("stage1_radius".to_string(), ParamValue::Length(20.0));
    old.insert("stage1_count".to_string(), ParamValue::Count(5));
    old.insert("stage1_step_x".to_string(), ParamValue::Length(0.0));
    old.insert("stage1_step_z".to_string(), ParamValue::Length(4.0));

    let migrated = migrate_params(&old);
    assert_eq!(migrated.int("stage1_mode"), StageMode::Turn.index(), "an old turning stage was read as a run");
    assert_eq!(migrated.num("stage1_rise"), 4.0, "the rise did not come across from the step along the axis");
    let helix = with(&[
        ("kind", ParamValue::Choice(HELIX)),
        ("helix_count", ParamValue::Count(5)),
        ("helix_angle", ParamValue::Angle(45.0)),
        ("helix_rise", ParamValue::Length(4.0)),
        ("helix_radius", ParamValue::Length(20.0)),
    ]);
    assert_eq!(instances(&migrated), instances(&helix));

    // The old mirror flag becomes the mirror mode.
    let mut old = default_params();
    old.remove("stage1_mode");
    old.insert("stage1_mirror".to_string(), ParamValue::Bool(true));
    assert_eq!(migrate_params(&old).int("stage1_mode"), StageMode::Mirror.index());
}
