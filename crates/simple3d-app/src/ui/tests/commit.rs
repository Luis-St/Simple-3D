//! Reading a typed value back, clamped and rounded.

use super::*;
use simple3d_core::primitive::{ParamKind, ParamValue};
use simple3d_core::unit::Unit;

#[test]
pub(crate) fn a_length_is_read_in_the_display_unit() {
    assert_eq!(
        commit_param("1.8", ParamKind::Length { min: 0.0 }, Unit::Metre, 0.0),
        Commit::Value(ParamValue::Length(1800.0))
    );
    assert_eq!(
        commit_param("1,8", ParamKind::Length { min: 0.0 }, Unit::Millimetre, 0.0),
        Commit::Value(ParamValue::Length(1.8))
    );
}

#[test]
pub(crate) fn garbage_reverts_rather_than_raising_a_dialog() {
    // Spec acceptance criterion 14.
    for bad in ["", "  ", "abc", "1.2.3", "--4", "NaN", "inf", "12mmm"] {
        assert_eq!(
            commit_param(bad, ParamKind::Length { min: 0.0 }, Unit::Millimetre, 10.0),
            Commit::Revert,
            "{bad:?}"
        );
        assert_eq!(commit_length(bad, Unit::Millimetre, 10.0), None, "{bad:?}");
        assert_eq!(commit_angle(bad, 10.0), None, "{bad:?}");
    }
}

#[test]
pub(crate) fn a_rotation_comes_back_as_the_direction_it_faces_not_the_turn_that_got_there() {
    // Issue 84: a field on a rotation reads [0, 360). Typing a whole turn
    // past where the body already stands leaves it standing there.
    assert_eq!(commit_angle("400", 0.0), Some(40.0));
    assert_eq!(commit_angle("360", 0.0), Some(0.0));
    assert_eq!(commit_angle("-90", 0.0), Some(270.0));
    // The relative forms are resolved first: a degree on from 359 is 0,
    // which is the case the issue names.
    assert_eq!(commit_angle("+1", 359.0), Some(0.0));
    assert_eq!(commit_angle("- 1", 0.0), Some(359.0));
}

#[test]
pub(crate) fn a_dimension_below_its_minimum_is_clamped_not_rejected() {
    let kind = ParamKind::Length { min: 1e-3 };
    assert_eq!(commit_param("0", kind, Unit::Millimetre, 4.0), Commit::Value(ParamValue::Length(1e-3)));
    assert_eq!(commit_param("-5", kind, Unit::Millimetre, 4.0), Commit::Value(ParamValue::Length(1e-3)));
    assert_eq!(commit_param("5", kind, Unit::Millimetre, 4.0), Commit::Value(ParamValue::Length(5.0)));
}

#[test]
pub(crate) fn a_count_is_rounded_and_clamped_to_its_range() {
    let kind = ParamKind::Count { min: 3, max: 128 };
    assert_eq!(commit_param("6", kind, Unit::Millimetre, 32.0), Commit::Value(ParamValue::Count(6)));
    assert_eq!(commit_param("6.7", kind, Unit::Millimetre, 32.0), Commit::Value(ParamValue::Count(7)));
    assert_eq!(commit_param("1", kind, Unit::Millimetre, 32.0), Commit::Value(ParamValue::Count(3)));
    assert_eq!(commit_param("999", kind, Unit::Millimetre, 32.0), Commit::Value(ParamValue::Count(128)));
    assert_eq!(commit_param("-4", kind, Unit::Millimetre, 32.0), Commit::Value(ParamValue::Count(3)));
}

#[test]
pub(crate) fn an_angle_is_clamped_to_its_range_and_stays_in_degrees() {
    let kind = ParamKind::Angle { min: 1.0, max: 360.0 };
    // Angles are always degrees regardless of the length unit.
    assert_eq!(commit_param("90", kind, Unit::Metre, 45.0), Commit::Value(ParamValue::Angle(90.0)));
    assert_eq!(commit_param("999", kind, Unit::Millimetre, 45.0), Commit::Value(ParamValue::Angle(360.0)));
    assert_eq!(commit_param("0", kind, Unit::Millimetre, 45.0), Commit::Value(ParamValue::Angle(1.0)));
}

#[test]
pub(crate) fn a_choice_index_cannot_run_off_the_end() {
    let kind = ParamKind::Choice { options: &["Across corners", "Across flats"] };
    assert_eq!(commit_param("1", kind, Unit::Millimetre, 0.0), Commit::Value(ParamValue::Choice(1)));
    assert_eq!(commit_param("7", kind, Unit::Millimetre, 0.0), Commit::Value(ParamValue::Choice(1)));
    assert_eq!(commit_param("-1", kind, Unit::Millimetre, 0.0), Commit::Value(ParamValue::Choice(0)));
}

#[test]
pub(crate) fn what_a_field_shows_round_trips_back_through_what_it_accepts() {
    for (value, unit) in [
        (ParamValue::Length(40.0), Unit::Millimetre),
        (ParamValue::Length(4.0), Unit::Metre),
        (ParamValue::Length(0.5), Unit::Centimetre),
        (ParamValue::Angle(180.0), Unit::Millimetre),
        (ParamValue::Count(6), Unit::Millimetre),
    ] {
        let shown = show_param(value, unit);
        let kind = match value {
            ParamValue::Length(_) => ParamKind::Length { min: 0.0 },
            ParamValue::Angle(_) => ParamKind::Angle { min: -360.0, max: 360.0 },
            ParamValue::Count(_) => ParamKind::Count { min: 0, max: 1000 },
            ParamValue::Choice(_) => ParamKind::Choice { options: &["a", "b"] },
            ParamValue::Bool(_) => ParamKind::Bool,
        };
        assert_eq!(
            commit_param(&shown, kind, unit, param_number(value)),
            Commit::Value(value),
            "{shown:?} in {unit:?}"
        );
    }
}

#[test]
pub(crate) fn shown_values_never_carry_floating_point_noise() {
    assert_eq!(show_param(ParamValue::Length(4.0), Unit::Metre), "0.004");
    assert_eq!(show_param(ParamValue::Length(0.1 + 0.2), Unit::Millimetre), "0.3");
    assert_eq!(show_param(ParamValue::Length(1800.0), Unit::Metre), "1.8");
}
