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
        kind: ParamKind::Angle { min: -360.0, max: 360.0, wrap: false },
        default: ParamValue::Angle(default),
        lock_group: 0,
        shown_when: Some(when),
    }
}

/// The axis a turning pattern turns about, whose perpendicular a radius is measured in.
pub(crate) const AXES: &[&str] = &["X", "Y", "Z"];

pub(crate) const fn axis(key: &'static str, when: (&'static str, u32)) -> ParamSpec {
    ParamSpec {
        key,
        label: "Axis",
        kind: ParamKind::Choice { options: AXES },
        // Z by default: rings and helices stand up on the build plate.
        default: ParamValue::Choice(2),
        lock_group: 0,
        shown_when: Some(when),
    }
}

/// What one custom stage does (issue 79): a unitless choice, so all stages share one label.
pub(crate) const fn does(key: &'static str, default: u32) -> ParamSpec {
    ParamSpec {
        key,
        label: "Does",
        kind: ParamKind::Choice { options: super::STAGE_MODES },
        default: ParamValue::Choice(default),
        lock_group: 0,
        shown_when: Some(("kind", super::CUSTOM)),
    }
}

/// How far a copy may be nudged (issue 79). Unbounded either way, since the scatter uses the
/// absolute value ([`Noise::of`](super::Noise::of)) and a field refusing `-5` looked broken.
pub(crate) const fn jitter(key: &'static str, label: &'static str) -> ParamSpec {
    ParamSpec {
        key,
        label,
        kind: ParamKind::Length { min: f64::NEG_INFINITY },
        default: ParamValue::Length(0.0),
        lock_group: 0,
        // Every kind: planks in a run want randomness too.
        shown_when: None,
    }
}

/// How many of something a stage has (its variations, issue 79); unlike a copy count it can be zero.
pub(crate) const fn tally(key: &'static str, label: &'static str, max: u32) -> ParamSpec {
    ParamSpec {
        key,
        label,
        kind: ParamKind::Count { min: 0, max },
        default: ParamValue::Count(0),
        lock_group: 0,
        shown_when: Some(("kind", super::CUSTOM)),
    }
}

/// How far a copy may be turned about one axis (issue 79).
pub(crate) const fn turn_jitter(key: &'static str, label: &'static str) -> ParamSpec {
    ParamSpec {
        key,
        label,
        kind: ParamKind::Angle { min: 0.0, max: 360.0, wrap: true },
        default: ParamValue::Angle(0.0),
        lock_group: 0,
        shown_when: None,
    }
}
