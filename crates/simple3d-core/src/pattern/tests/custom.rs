//! A custom rule saying what each built-in kind says.

use super::*;
use crate::primitive::{ParamValue, Params};
use simple3d_geom::Vec3;

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
        ("stage1_count", ParamValue::Count(3)),
        ("stage1_step_x", ParamValue::Length(10.0)),
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
        ("stage1_count", ParamValue::Count(6)),
        ("stage1_turn", ParamValue::Angle(60.0)),
        ("stage1_radius", ParamValue::Length(25.0)),
        ("stage1_step_x", ParamValue::Length(0.0)),
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
        ("stage1_count", ParamValue::Count(5)),
        ("stage1_turn", ParamValue::Angle(45.0)),
        ("stage1_radius", ParamValue::Length(20.0)),
        ("stage1_step_x", ParamValue::Length(0.0)),
        ("stage1_step_z", ParamValue::Length(4.0)),
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
        ("stage1_count", ParamValue::Count(7)),
        ("stage1_turn", ParamValue::Angle(30.0)),
        ("stage1_radius", ParamValue::Length(10.0)),
        ("stage1_growth", ParamValue::Length(5.0)),
        ("stage1_step_x", ParamValue::Length(0.0)),
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
        ("stage1_mirror", ParamValue::Bool(true)),
        ("stage1_axis", ParamValue::Choice(0)),
    ]);
    assert_eq!(instances(&as_stage), instances(&mirrored));

    let twice = with(&[
        ("kind", ParamValue::Choice(CUSTOM)),
        ("stages", ParamValue::Count(2)),
        ("stage1_mirror", ParamValue::Bool(true)),
        ("stage1_axis", ParamValue::Choice(0)),
        ("stage2_mirror", ParamValue::Bool(true)),
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
        ("stage1_count", ParamValue::Count(3)),
        ("stage1_step_x", ParamValue::Length(10.0)),
        ("stage2_count", ParamValue::Count(4)),
        ("stage2_turn", ParamValue::Angle(90.0)),
        ("stage2_step_x", ParamValue::Length(0.0)),
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
        params.insert(keys.count.to_string(), ParamValue::Count(512));
        params.insert(keys.step[0].to_string(), ParamValue::Length(1.0));
    }
    let (wanted, made) = instance_count(&params);
    assert_eq!(wanted, 512usize.pow(4));
    assert_eq!(made, MAX_INSTANCES);
    assert_eq!(instances(&params).len(), MAX_INSTANCES);
}

/// Only the stages in use are shown, a mirror stage shows nothing but the
/// plane it reflects across, and no stage at all is shown for another kind.
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

    let mirrored = with(&[("kind", ParamValue::Choice(CUSTOM)), ("stage1_mirror", ParamValue::Bool(true))]);
    assert!(shown(&mirrored).contains(&"stage1_axis"), "a mirror stage still chooses its plane");
    assert!(!shown(&mirrored).contains(&"stage1_step_x"), "a mirror stage has no run to place");
}

/// Every key the stage table names has to be a parameter that exists, or a
/// handle would drive a number nothing shows and nothing saves.
#[test]
pub(crate) fn every_stage_key_is_a_parameter_of_its_own() {
    let known: Vec<&'static str> = PARAMS.iter().map(|p| p.key).collect();
    for key in custom_keys() {
        assert!(known.contains(&key), "{key} is named by the stage table but is not a parameter");
    }
    assert_eq!(custom_keys().len(), 1 + MAX_STAGES * 9);
}
