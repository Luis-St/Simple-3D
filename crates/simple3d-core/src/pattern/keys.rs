//! The parameter names a pattern's numbers are stored under.

use crate::primitive::{ParamKind, ParamSpec, ParamValue};

pub(crate) const fn length(
    key: &'static str,
    label: &'static str,
    default: f64,
    when: (&'static str, u32),
) -> ParamSpec {
    ParamSpec {
        key,
        label,
        kind: ParamKind::Length { min: f64::NEG_INFINITY },
        default: ParamValue::Length(default),
        lock_group: 0,
        shown_when: Some(when),
    }
}

pub(crate) const fn count(
    key: &'static str,
    label: &'static str,
    default: u32,
    when: (&'static str, u32),
) -> ParamSpec {
    ParamSpec {
        key,
        label,
        kind: ParamKind::Count { min: 1, max: 512 },
        default: ParamValue::Count(default),
        lock_group: 0,
        shown_when: Some(when),
    }
}

pub(crate) const fn angle(
    key: &'static str,
    label: &'static str,
    default: f64,
    when: (&'static str, u32),
) -> ParamSpec {
    ParamSpec {
        key,
        label,
        kind: ParamKind::Angle { min: -360.0, max: 360.0 },
        default: ParamValue::Angle(default),
        lock_group: 0,
        shown_when: Some(when),
    }
}

pub(crate) const fn flag(key: &'static str, label: &'static str, when: (&'static str, u32)) -> ParamSpec {
    ParamSpec {
        key,
        label,
        kind: ParamKind::Bool,
        default: ParamValue::Bool(false),
        lock_group: 0,
        shown_when: Some(when),
    }
}

/// The axis a turning pattern turns about, and whose perpendicular a radius is
/// measured in.
pub(crate) const AXES: &[&str] = &["X", "Y", "Z"];

pub(crate) const fn axis(key: &'static str, when: (&'static str, u32)) -> ParamSpec {
    ParamSpec {
        key,
        label: "Axis",
        kind: ParamKind::Choice { options: AXES },
        // Z by default: a ring or a helix stands up, which is what a build plate
        // wants.
        default: ParamValue::Choice(2),
        lock_group: 0,
        shown_when: Some(when),
    }
}
