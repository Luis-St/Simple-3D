//! Where each built-in kind puts its copies.

use super::*;
use crate::primitive::ParamValue;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn a_linear_pattern_steps_its_copies_along_a_fixed_offset() {
    let params = with(&[
        ("kind", ParamValue::Choice(LINEAR)),
        ("count", ParamValue::Count(4)),
        ("step_x", ParamValue::Length(10.0)),
        ("step_y", ParamValue::Length(0.0)),
        ("step_z", ParamValue::Length(0.0)),
    ]);
    let copies = instances(&params);
    assert_eq!(copies.len(), 4);
    assert!((copies[0].xform.t - Vec3::ZERO).length() < 1e-9, "the first copy is the original");
    assert!((copies[3].xform.t - Vec3::new(30.0, 0.0, 0.0)).length() < 1e-9);
    assert!(copies.iter().all(|c| !c.mirrored));
}

#[test]
pub(crate) fn a_grid_pattern_fills_a_lattice() {
    let params = with(&[
        ("kind", ParamValue::Choice(GRID)),
        ("grid_x", ParamValue::Count(3)),
        ("grid_y", ParamValue::Count(2)),
        ("grid_z", ParamValue::Count(1)),
        ("grid_step_x", ParamValue::Length(10.0)),
        ("grid_step_y", ParamValue::Length(5.0)),
    ]);
    let copies = instances(&params);
    assert_eq!(copies.len(), 6, "3 x 2 x 1");
    // The far corner of the lattice.
    assert!(copies.iter().any(|c| (c.xform.t - Vec3::new(20.0, 5.0, 0.0)).length() < 1e-9));
}

#[test]
pub(crate) fn a_circular_pattern_spaces_a_full_turn_by_the_count() {
    let params = with(&[
        ("kind", ParamValue::Choice(CIRCULAR)),
        ("circ_count", ParamValue::Count(4)),
        ("circ_span", ParamValue::Angle(360.0)),
        ("circ_radius", ParamValue::Length(10.0)),
        ("circ_axis", ParamValue::Choice(2)),
    ]);
    let copies = instances(&params);
    assert_eq!(copies.len(), 4);
    // Radius out along +X (the radial axis for a turn about Z), and a quarter
    // turn brings the second copy round to +Y.
    assert!((copies[0].xform.point(Vec3::ZERO) - Vec3::new(10.0, 0.0, 0.0)).length() < 1e-6);
    assert!(
        (copies[1].xform.point(Vec3::ZERO) - Vec3::new(0.0, 10.0, 0.0)).length() < 1e-6,
        "{:?}",
        copies[1].xform.point(Vec3::ZERO)
    );
}

#[test]
pub(crate) fn a_partial_circular_pattern_places_both_ends() {
    let params = with(&[
        ("kind", ParamValue::Choice(CIRCULAR)),
        ("circ_count", ParamValue::Count(3)),
        ("circ_span", ParamValue::Angle(90.0)),
        ("circ_radius", ParamValue::Length(10.0)),
        ("circ_axis", ParamValue::Choice(2)),
    ]);
    let copies = instances(&params);
    // Three copies over 90 degrees: at 0, 45 and 90.
    assert!((copies[2].xform.point(Vec3::ZERO) - Vec3::new(0.0, 10.0, 0.0)).length() < 1e-6);
}

#[test]
pub(crate) fn a_mirror_pattern_reflects_and_marks_the_copy_for_a_winding_flip() {
    let params = with(&[("kind", ParamValue::Choice(MIRROR)), ("mirror_axis", ParamValue::Choice(0))]);
    let copies = instances(&params);
    assert_eq!(copies.len(), 2);
    assert!(!copies[0].mirrored, "the original is not a reflection");
    assert!(copies[1].mirrored, "the reflection must be flagged for a winding flip");
    // A point at +X reflects to -X across the X-normal plane.
    assert!((copies[1].xform.point(Vec3::new(3.0, 1.0, 2.0)) - Vec3::new(-3.0, 1.0, 2.0)).length() < 1e-9);
}

#[test]
pub(crate) fn a_helix_turns_and_rises_together() {
    let params = with(&[
        ("kind", ParamValue::Choice(HELIX)),
        ("helix_count", ParamValue::Count(5)),
        ("helix_angle", ParamValue::Angle(90.0)),
        ("helix_rise", ParamValue::Length(4.0)),
        ("helix_radius", ParamValue::Length(10.0)),
        ("helix_axis", ParamValue::Choice(2)),
    ]);
    let copies = instances(&params);
    assert_eq!(copies.len(), 5);
    // The fourth copy: three 90-degree turns (back to +Y from +X twice round)
    // and three rises of 4.
    let p = copies[3].xform.point(Vec3::ZERO);
    assert!((p.z - 12.0).abs() < 1e-6, "it did not rise: {p:?}");
    // Radius held: the horizontal distance stays the radius.
    assert!(((p.x * p.x + p.y * p.y).sqrt() - 10.0).abs() < 1e-6, "the radius drifted: {p:?}");
}

#[test]
pub(crate) fn a_spiral_grows_its_radius_each_copy() {
    let params = with(&[
        ("kind", ParamValue::Choice(SPIRAL)),
        ("spiral_count", ParamValue::Count(4)),
        ("spiral_angle", ParamValue::Angle(0.0)),
        ("spiral_radius", ParamValue::Length(5.0)),
        ("spiral_growth", ParamValue::Length(3.0)),
        ("spiral_axis", ParamValue::Choice(2)),
    ]);
    let copies = instances(&params);
    // No angle, so every copy is out along +X, at a growing radius.
    assert!((copies[0].xform.point(Vec3::ZERO) - Vec3::new(5.0, 0.0, 0.0)).length() < 1e-6);
    assert!((copies[3].xform.point(Vec3::ZERO) - Vec3::new(14.0, 0.0, 0.0)).length() < 1e-6);
}
