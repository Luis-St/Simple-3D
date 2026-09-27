//! The handles a pattern is dragged by.

use super::*;
use crate::primitive::{ParamValue, ParamsExt};
use simple3d_geom::Vec3;

#[test]
pub(crate) fn a_grip_sits_on_the_copy_it_places_and_writes_back_the_number_it_shows() {
    // A grip sits where its number puts it, so dragging and reading back are the same operation.
    let mut params = with(&[
        ("kind", ParamValue::Choice(LINEAR)),
        ("count", ParamValue::Count(4)),
        ("step_x", ParamValue::Length(10.0)),
        ("step_y", ParamValue::Length(0.0)),
        ("step_z", ParamValue::Length(0.0)),
    ]);
    let spacing = grip(&params, "Spacing").expect("a run of four has a spacing to lay out");
    // At the last copy: three gaps of 10.
    assert!((spacing.at - Vec3::new(30.0, 0.0, 0.0)).length() < 1e-9, "{:?}", spacing.at);
    // Dragged out to 60, the gaps become 20 each.
    apply_grip(&mut params, &spacing, 60.0);
    assert_eq!(params.get("step_x"), Some(&ParamValue::Length(20.0)));
    // The grip follows the copy it marks.
    assert!((grip(&params, "Spacing").unwrap().at - Vec3::new(60.0, 0.0, 0.0)).length() < 1e-9);

    // The copies grip sits one step past the last copy.
    let copies = grip(&params, "Copies").unwrap();
    assert!((copies.at - Vec3::new(80.0, 0.0, 0.0)).length() < 1e-9, "{:?}", copies.at);
    apply_grip(&mut params, &copies, 140.0);
    assert_eq!(params.get("count"), Some(&ParamValue::Count(7)), "seven 20 mm steps reach 140");
}

#[test]
pub(crate) fn a_run_that_steps_diagonally_lengthens_along_its_own_diagonal() {
    let mut params = with(&[
        ("kind", ParamValue::Choice(LINEAR)),
        ("count", ParamValue::Count(3)),
        ("step_x", ParamValue::Length(3.0)),
        ("step_y", ParamValue::Length(4.0)),
        ("step_z", ParamValue::Length(0.0)),
    ]);
    let spacing = grip(&params, "Spacing").unwrap();
    // Two gaps of a 3-4-5 run: ten out along the diagonal.
    assert!((spacing.at - Vec3::new(6.0, 8.0, 0.0)).length() < 1e-9);
    apply_grip(&mut params, &spacing, 20.0);
    let step = Vec3::new(params.num("step_x"), params.num("step_y"), params.num("step_z"));
    assert!((step - Vec3::new(6.0, 8.0, 0.0)).length() < 1e-9, "straightened onto X: {step:?}");
}

#[test]
pub(crate) fn a_count_grip_never_writes_a_number_the_property_editor_would_refuse() {
    let mut params = with(&[("kind", ParamValue::Choice(LINEAR)), ("step_x", ParamValue::Length(10.0))]);
    let copies = grip(&params, "Copies").unwrap();
    // Dragged back through the origin, one copy is the floor.
    apply_grip(&mut params, &copies, -70.0);
    assert_eq!(params.get("count"), Some(&ParamValue::Count(1)));
    // The far end stops at the parameter's own 512.
    apply_grip(&mut params, &copies, 1_000_000.0);
    assert_eq!(params.get("count"), Some(&ParamValue::Count(512)));
}

#[test]
pub(crate) fn every_kind_that_places_copies_offers_a_grip_for_each_number_that_places_them() {
    // Issue 67: every kind's grips by name, each where its parameter says, or it would jump when grabbed.
    let grid = with(&[
        ("kind", ParamValue::Choice(GRID)),
        ("grid_x", ParamValue::Count(3)),
        ("grid_y", ParamValue::Count(2)),
        ("grid_z", ParamValue::Count(1)),
        ("grid_step_x", ParamValue::Length(10.0)),
        ("grid_step_y", ParamValue::Length(5.0)),
        ("grid_step_z", ParamValue::Length(4.0)),
    ]);
    assert_eq!(labels(&grid), vec!["Column spacing", "Columns", "Row spacing", "Rows", "Layers"]);
    assert!((grip(&grid, "Column spacing").unwrap().at - Vec3::new(20.0, 0.0, 0.0)).length() < 1e-9);
    assert!((grip(&grid, "Rows").unwrap().at - Vec3::new(0.0, 10.0, 0.0)).length() < 1e-9);
    // One layer has no spacing yet, but the count grip is there to pull out a second.
    assert!((grip(&grid, "Layers").unwrap().at - Vec3::new(0.0, 0.0, 4.0)).length() < 1e-9);

    let ring = with(&[
        ("kind", ParamValue::Choice(CIRCULAR)),
        ("circ_radius", ParamValue::Length(10.0)),
        ("circ_span", ParamValue::Angle(90.0)),
        ("circ_axis", ParamValue::Choice(2)),
    ]);
    assert_eq!(labels(&ring), vec!["Radius", "Span"]);
    assert!((grip(&ring, "Radius").unwrap().at - Vec3::new(10.0, 0.0, 0.0)).length() < 1e-9);
    // A quarter turn round, outside the copies so a full turn does not overlap the radius grip.
    let span = grip(&ring, "Span").unwrap();
    assert!((span.at - Vec3::new(0.0, 13.0, 0.0)).length() < 1e-6, "{:?}", span.at);

    let helix = with(&[
        ("kind", ParamValue::Choice(HELIX)),
        ("helix_count", ParamValue::Count(5)),
        ("helix_radius", ParamValue::Length(10.0)),
        ("helix_rise", ParamValue::Length(4.0)),
        ("helix_axis", ParamValue::Choice(2)),
    ]);
    assert_eq!(labels(&helix), vec!["Radius", "Rise", "Copies", "Turn per copy"]);
    // The twist grip sits on the second copy, so its angle is the turn per copy.
    let turn = grip(&helix, "Turn per copy").unwrap();
    let Drive::Angle { per, .. } = turn.drive else { panic!("a turn grip must turn") };
    assert!((per - 1.0).abs() < 1e-9);
    assert!((turn.at.z - 4.0).abs() < 1e-9, "it left the second copy's level: {:?}", turn.at);
    // Both height grips ride the axis: one at the top copy, one a rise above it.
    assert!((grip(&helix, "Rise").unwrap().at - Vec3::new(0.0, 0.0, 16.0)).length() < 1e-9);
    assert!((grip(&helix, "Copies").unwrap().at - Vec3::new(0.0, 0.0, 20.0)).length() < 1e-9);

    let spiral = with(&[
        ("kind", ParamValue::Choice(SPIRAL)),
        ("spiral_count", ParamValue::Count(4)),
        ("spiral_radius", ParamValue::Length(5.0)),
        ("spiral_growth", ParamValue::Length(3.0)),
        ("spiral_rise", ParamValue::Length(0.0)),
        ("spiral_axis", ParamValue::Choice(2)),
    ]);
    // A ruler out from the centre; no rise grip on a flat spiral.
    assert_eq!(labels(&spiral), vec!["Start radius", "Radius per copy", "Copies", "Turn per copy"]);
    assert!((grip(&spiral, "Radius per copy").unwrap().at - Vec3::new(14.0, 0.0, 0.0)).length() < 1e-9);
    assert!((grip(&spiral, "Copies").unwrap().at - Vec3::new(17.0, 0.0, 0.0)).length() < 1e-9);

    // A mirror has no distance or count, so no grips.
    assert!(labels(&with(&[("kind", ParamValue::Choice(MIRROR))])).is_empty());
}

#[test]
pub(crate) fn a_grip_is_never_offered_on_the_patterns_own_origin() {
    // No grip on the pattern's origin, where the move manipulator sits; a zero-length run puts every grip there.
    let flat = with(&[
        ("kind", ParamValue::Choice(LINEAR)),
        ("count", ParamValue::Count(4)),
        ("step_x", ParamValue::Length(0.0)),
        ("step_y", ParamValue::Length(0.0)),
        ("step_z", ParamValue::Length(0.0)),
    ]);
    assert!(labels(&flat).is_empty());
    // A zero-radius ring offers the radius, but no span grip at the centre.
    let point = with(&[("kind", ParamValue::Choice(CIRCULAR)), ("circ_radius", ParamValue::Length(0.0))]);
    assert!(labels(&point).is_empty());
}

#[test]
pub(crate) fn a_turning_grip_follows_the_axis_the_pattern_turns_about() {
    for axis in 0..3u32 {
        let ring = with(&[
            ("kind", ParamValue::Choice(CIRCULAR)),
            ("circ_radius", ParamValue::Length(10.0)),
            ("circ_span", ParamValue::Angle(360.0)),
            ("circ_axis", ParamValue::Choice(axis)),
        ]);
        let span = grip(&ring, "Span").expect("a ring with a radius has a span to drag");
        let Drive::Angle { axis: about, .. } = span.drive else { panic!("a span grip must turn") };
        assert_eq!(about, axis as usize);
        // It rides in the turn's plane, so the axis component is zero.
        let along = match axis {
            0 => span.at.x,
            1 => span.at.y,
            _ => span.at.z,
        };
        assert!(along.abs() < 1e-9, "the span grip left the plane of its own ring: {:?}", span.at);
    }
}

#[test]
pub(crate) fn every_parameter_belongs_to_a_real_kind_and_the_kinds_are_all_listed() {
    // A parameter gated on a nonexistent kind would never show.
    for spec in PARAMS {
        if let Some((key, value)) = spec.shown_when {
            assert_eq!(key, "kind", "{}", spec.key);
            assert!((value as usize) < KINDS.len(), "{} is gated on a kind that does not exist", spec.key);
        }
    }
    // Every kind has parameters of its own, or it would be a dead choice.
    for k in 0..KINDS.len() as u32 {
        let mut params = default_params();
        params.insert("kind".into(), ParamValue::Choice(k));
        assert!(!instances(&params).is_empty(), "kind {k} produced no copies");
    }
}
