//! A custom rule saying what each built-in kind says.

use super::*;
use crate::primitive::{ParamValue, Params, ParamsExt};
use simple3d_geom::Vec3;

/// What a stage does, as the choice the parameter holds.
fn mode(mode: StageMode) -> ParamValue {
    ParamValue::Choice(mode.index())
}

/// A custom rule with one stage set to step along X is a linear pattern.
/// Not "close to one" -- the same transforms, copy for copy, which is what
/// says the stage model is the rule the fixed kinds are special cases of
/// rather than a seventh thing that happens to look similar (issue 67).
#[test]
pub(crate) fn one_custom_stage_says_exactly_what_a_linear_pattern_says() {
    let linear = with(&[
        ("kind", ParamValue::Choice(LINEAR)),
        ("count", ParamValue::Count(4)),
        ("step_x", ParamValue::Length(10.0)),
    ]);
    let custom = with(&[
        ("kind", ParamValue::Choice(CUSTOM)),
        ("stages", ParamValue::Count(1)),
        ("stage1_mode", mode(StageMode::Move)),
        ("stage1_count", ParamValue::Count(4)),
        ("stage1_step_x", ParamValue::Length(10.0)),
    ]);
    assert_eq!(instances(&custom), instances(&linear));
}

/// Two stages stepping along two axes are a grid, in the same order.
#[test]
pub(crate) fn two_custom_stages_say_exactly_what_a_grid_says() {
    let grid = with(&[
        ("kind", ParamValue::Choice(GRID)),
        ("grid_x", ParamValue::Count(3)),
        ("grid_y", ParamValue::Count(2)),
        ("grid_z", ParamValue::Count(1)),
        ("grid_step_x", ParamValue::Length(10.0)),
        ("grid_step_y", ParamValue::Length(5.0)),
    ]);
    let custom = with(&[
        ("kind", ParamValue::Choice(CUSTOM)),
        ("stages", ParamValue::Count(2)),
        ("stage1_mode", mode(StageMode::Move)),
        ("stage1_count", ParamValue::Count(3)),
        ("stage1_step_x", ParamValue::Length(10.0)),
        ("stage2_mode", mode(StageMode::Move)),
        ("stage2_count", ParamValue::Count(2)),
        ("stage2_step_y", ParamValue::Length(5.0)),
    ]);
    assert_eq!(instances(&custom), instances(&grid));
}

/// A stage that turns at a radius is a ring, and a stage that turns while
/// rising is a helix.
#[test]
pub(crate) fn a_turning_stage_says_what_a_ring_and_a_helix_say() {
    let ring = with(&[
        ("kind", ParamValue::Choice(CIRCULAR)),
        ("circ_count", ParamValue::Count(6)),
        ("circ_span", ParamValue::Angle(360.0)),
        ("circ_radius", ParamValue::Length(25.0)),
    ]);
    let as_stage = with(&[
        ("kind", ParamValue::Choice(CUSTOM)),
        ("stage1_mode", mode(StageMode::Turn)),
        ("stage1_count", ParamValue::Count(6)),
        ("stage1_turn", ParamValue::Angle(60.0)),
        ("stage1_radius", ParamValue::Length(25.0)),
    ]);
    assert_eq!(instances(&as_stage), instances(&ring));

    let helix = with(&[
        ("kind", ParamValue::Choice(HELIX)),
        ("helix_count", ParamValue::Count(5)),
        ("helix_angle", ParamValue::Angle(45.0)),
        ("helix_rise", ParamValue::Length(4.0)),
        ("helix_radius", ParamValue::Length(20.0)),
    ]);
    let as_stage = with(&[
        ("kind", ParamValue::Choice(CUSTOM)),
        ("stage1_mode", mode(StageMode::Turn)),
        ("stage1_count", ParamValue::Count(5)),
        ("stage1_turn", ParamValue::Angle(45.0)),
        ("stage1_radius", ParamValue::Length(20.0)),
        ("stage1_rise", ParamValue::Length(4.0)),
    ]);
    assert_eq!(instances(&as_stage), instances(&helix));
}

/// A stage that grows its radius while it turns is a spiral.
#[test]
pub(crate) fn a_growing_stage_says_what_a_spiral_says() {
    let spiral = with(&[
        ("kind", ParamValue::Choice(SPIRAL)),
        ("spiral_count", ParamValue::Count(7)),
        ("spiral_angle", ParamValue::Angle(30.0)),
        ("spiral_radius", ParamValue::Length(10.0)),
        ("spiral_growth", ParamValue::Length(5.0)),
        ("spiral_rise", ParamValue::Length(0.0)),
    ]);
    let as_stage = with(&[
        ("kind", ParamValue::Choice(CUSTOM)),
        ("stage1_mode", mode(StageMode::Turn)),
        ("stage1_count", ParamValue::Count(7)),
        ("stage1_turn", ParamValue::Angle(30.0)),
        ("stage1_radius", ParamValue::Length(10.0)),
        ("stage1_growth", ParamValue::Length(5.0)),
    ]);
    assert_eq!(instances(&as_stage), instances(&spiral));
}

/// A mirror stage is a mirror pattern, and mirroring twice points the faces
/// back outward rather than leaving them inside out.
#[test]
pub(crate) fn a_mirror_stage_reflects_and_two_of_them_cancel() {
    let mirrored = with(&[("kind", ParamValue::Choice(MIRROR)), ("mirror_axis", ParamValue::Choice(0))]);
    let as_stage = with(&[
        ("kind", ParamValue::Choice(CUSTOM)),
        ("stage1_mode", mode(StageMode::Mirror)),
        ("stage1_axis", ParamValue::Choice(0)),
    ]);
    assert_eq!(instances(&as_stage), instances(&mirrored));

    let twice = with(&[
        ("kind", ParamValue::Choice(CUSTOM)),
        ("stages", ParamValue::Count(2)),
        ("stage1_mode", mode(StageMode::Mirror)),
        ("stage1_axis", ParamValue::Choice(0)),
        ("stage2_mode", mode(StageMode::Mirror)),
        ("stage2_axis", ParamValue::Choice(1)),
    ]);
    let copies = instances(&twice);
    assert_eq!(copies.len(), 4);
    assert_eq!(copies.iter().filter(|c| c.mirrored).count(), 2, "a reflection of a reflection points out again");
}

/// The point of stages: a later one repeats what the earlier ones *made*.
/// A row of three, turned four times about Z, is four rows standing round a
/// centre -- not four copies of the first shape and three of nothing.
#[test]
pub(crate) fn a_later_stage_repeats_what_the_earlier_ones_made() {
    let params = with(&[
        ("kind", ParamValue::Choice(CUSTOM)),
        ("stages", ParamValue::Count(2)),
        ("stage1_mode", mode(StageMode::Move)),
        ("stage1_count", ParamValue::Count(3)),
        ("stage1_step_x", ParamValue::Length(10.0)),
        ("stage2_mode", mode(StageMode::Turn)),
        ("stage2_count", ParamValue::Count(4)),
        ("stage2_turn", ParamValue::Angle(90.0)),
    ]);
    let copies = instances(&params);
    assert_eq!(copies.len(), 12);
    assert_eq!(instance_count(&params), (12, 12));
    // The row runs out along X; a quarter turn about Z carries it onto Y,
    // so the far end of the second row is 20 mm up the Y axis.
    assert!(
        copies.iter().any(|c| (c.xform.t - Vec3::new(0.0, 20.0, 0.0)).length() < 1e-9),
        "the second stage turned the copies rather than the row"
    );
}

/// Four stages of 512 multiply to more transforms than there is memory for,
/// so the cap has to bite while the copies are being built.
#[test]
pub(crate) fn a_custom_rule_cannot_ask_for_more_copies_than_anything_can_draw() {
    let mut params = with(&[("kind", ParamValue::Choice(CUSTOM)), ("stages", ParamValue::Count(4))]);
    for keys in &STAGES {
        params.insert(keys.mode.to_string(), mode(StageMode::Move));
        params.insert(keys.count.to_string(), ParamValue::Count(512));
        params.insert(keys.step[0].to_string(), ParamValue::Length(1.0));
    }
    let (wanted, made) = instance_count(&params);
    assert_eq!(wanted, 512usize.pow(4));
    assert_eq!(made, MAX_INSTANCES);
    assert_eq!(instances(&params).len(), MAX_INSTANCES);
}

/// Only the stages in use are shown, and a stage shows only the numbers the
/// thing it does actually needs -- which is what a stage's mode is for
/// (issue 79). No stage at all is shown for another kind.
#[test]
pub(crate) fn a_stages_fields_are_shown_only_while_that_stage_is_in_use() {
    let shown = |params: &Params| -> Vec<&'static str> {
        PARAMS.iter().filter(|p| param_visible(p, params)).map(|p| p.key).collect()
    };
    let linear = with(&[("kind", ParamValue::Choice(LINEAR))]);
    assert!(shown(&linear).iter().all(|k| !k.starts_with("stage")));

    let one = with(&[("kind", ParamValue::Choice(CUSTOM)), ("stages", ParamValue::Count(1))]);
    assert!(shown(&one).contains(&"stage1_count"));
    assert!(!shown(&one).contains(&"stage2_count"), "a stage that is not in use was still shown");

    // A run has a step and nothing that turns.
    let run = with(&[("kind", ParamValue::Choice(CUSTOM)), ("stage1_mode", mode(StageMode::Move))]);
    assert!(shown(&run).contains(&"stage1_step_x"));
    for key in ["stage1_turn", "stage1_radius", "stage1_growth", "stage1_rise", "stage1_axis"] {
        assert!(!shown(&run).contains(&key), "a run that goes straight was still offered {key}");
    }

    // A turn has all of those and no step.
    let turn = with(&[("kind", ParamValue::Choice(CUSTOM)), ("stage1_mode", mode(StageMode::Turn))]);
    for key in ["stage1_turn", "stage1_radius", "stage1_growth", "stage1_rise", "stage1_axis", "stage1_count"] {
        assert!(shown(&turn).contains(&key), "a turning stage was not offered {key}");
    }
    assert!(!shown(&turn).contains(&"stage1_step_x"), "a turning stage was offered a run to place");

    // And a mirror is a plane and two copies.
    let mirrored = with(&[("kind", ParamValue::Choice(CUSTOM)), ("stage1_mode", mode(StageMode::Mirror))]);
    assert!(shown(&mirrored).contains(&"stage1_axis"), "a mirror stage still chooses its plane");
    assert!(!shown(&mirrored).contains(&"stage1_step_x"), "a mirror stage has no run to place");
    assert!(!shown(&mirrored).contains(&"stage1_count"), "a mirror stage is always two copies");
}

/// Every key the stage table names has to be a parameter that exists, or a
/// handle would drive a number nothing shows and nothing saves.
#[test]
pub(crate) fn every_stage_key_is_a_parameter_of_its_own() {
    let known: Vec<&'static str> = PARAMS.iter().map(|p| p.key).collect();
    for key in custom_keys() {
        assert!(known.contains(&key), "{key} is named by the stage table but is not a parameter");
    }
    assert_eq!(custom_keys().len(), 1 + MAX_STAGES * STAGE_KEY_COUNT);
    // The rule is the stages and nothing else. The scatter can travel on the
    // shelf with it, but only when it is asked to (see `pattern_library`), so
    // it is not one of the rule's own keys.
    for key in noise_keys() {
        assert!(!custom_keys().contains(key), "{key} is one of the rule's keys but is the scatter's");
        assert!(known.contains(key), "{key} is named by the noise but is not a parameter");
    }
}

/// Each of the six fixed kinds can be written out as the stages that say the
/// same thing, which is what makes them templates for a rule of one's own
/// (issue 79). Not "something similar" -- the very same copies.
#[test]
pub(crate) fn every_fixed_kind_can_be_started_from_as_a_template() {
    for kind in 0..CUSTOM {
        // The kind's own numbers, moved off their defaults so the template is
        // proved to carry them rather than to happen to agree.
        let mut fixed = with(&[
            ("kind", ParamValue::Choice(kind)),
            ("count", ParamValue::Count(4)),
            ("step_x", ParamValue::Length(11.0)),
            ("step_y", ParamValue::Length(3.0)),
            ("grid_x", ParamValue::Count(3)),
            ("grid_y", ParamValue::Count(2)),
            ("grid_z", ParamValue::Count(2)),
            ("circ_count", ParamValue::Count(5)),
            ("circ_span", ParamValue::Angle(180.0)),
            ("circ_radius", ParamValue::Length(17.0)),
            ("mirror_axis", ParamValue::Choice(1)),
            ("helix_count", ParamValue::Count(6)),
            ("helix_rise", ParamValue::Length(3.5)),
            ("spiral_count", ParamValue::Count(9)),
            ("spiral_rise", ParamValue::Length(2.0)),
        ]);
        let laid_out = instances(&fixed);

        use_as_template(&mut fixed, kind);
        assert_eq!(fixed.int("kind"), CUSTOM, "the template did not switch the pattern to its own rule");
        assert_eq!(
            instances(&fixed),
            laid_out,
            "starting from {} laid the copies out somewhere else",
            KINDS[kind as usize]
        );
    }
}

/// A rule saved or filed before a stage said what it does still lays its
/// copies down where it always did (issue 79).
#[test]
pub(crate) fn a_rule_from_before_the_stage_modes_lays_out_the_same_copies() {
    // The old shape of a helix stage: a turn, a radius, and the rise written
    // as the step along the stage's own axis.
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

    // And the mirror flag that a stage used to carry becomes the mode that
    // replaced it.
    let mut old = default_params();
    old.remove("stage1_mode");
    old.insert("stage1_mirror".to_string(), ParamValue::Bool(true));
    assert_eq!(migrate_params(&old).int("stage1_mode"), StageMode::Mirror.index());
}
