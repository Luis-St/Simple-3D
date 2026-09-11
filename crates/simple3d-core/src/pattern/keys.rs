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

/// The axis a turning pattern turns about, and whose perpendicular a radius is
/// measured in.
pub(crate) const AXES: &[&str] = &["X", "Y", "Z"];

/// What the scatter's turn is about: one of the three axes, or all of them at
/// once (issue 79). The fourth is an index past the three a turn is ever
/// *about*, which is how [`Noise`](super::Noise) tells it apart.
pub(crate) const NOISE_AXES: &[&str] = &["X", "Y", "Z", "All"];

/// A whole number of percent, for a size that changes copy by copy (issue 79).
///
/// A count rather than a number of its own kind: a percentage is read and
/// typed and scrubbed exactly as a count is, and whole percent is finer than a
/// change anyone lays out by eye. The sign is in the number itself -- 100 is no
/// change, 90 a tenth smaller -- because a count has none.
pub(crate) const fn percent(
    key: &'static str,
    label: &'static str,
    default: u32,
    min: u32,
    max: u32,
    when: (&'static str, u32),
) -> ParamSpec {
    ParamSpec {
        key,
        label,
        kind: ParamKind::Count { min, max },
        default: ParamValue::Count(default),
        lock_group: 0,
        shown_when: Some(when),
    }
}

/// How many copies a stage's shift runs over before it starts again (issue
/// 79). Two at the least: a cycle of one shifts nothing, and a field that
/// accepted it would be a way to switch the shift off that looks like a number.
pub(crate) const fn cycle(
    key: &'static str,
    label: &'static str,
    default: u32,
    when: (&'static str, u32),
) -> ParamSpec {
    ParamSpec {
        key,
        label,
        kind: ParamKind::Count { min: 2, max: 64 },
        default: ParamValue::Count(default),
        lock_group: 0,
        shown_when: Some(when),
    }
}

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

/// What one stage of a custom rule does (issue 79). A choice like the axis, and
/// like the axis it carries no unit and no number, so every stage's may share
/// one label.
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

/// How far a copy may be nudged off where the rule puts it (issue 79).
///
/// Unbounded either way, like a position and unlike a dimension. It is read as
/// a distance -- the scatter runs both ways whatever the sign, which is what
/// [`Noise::of`](super::Noise::of) takes the absolute value for -- but a field
/// that snapped a typed `-5` back to zero was the one number in the panel that
/// refused a minus sign, and refused it without saying why.
pub(crate) const fn jitter(key: &'static str, label: &'static str) -> ParamSpec {
    ParamSpec {
        key,
        label,
        kind: ParamKind::Length { min: f64::NEG_INFINITY },
        default: ParamValue::Length(0.0),
        lock_group: 0,
        // Every kind, not one: a run of planks wants a little randomness as much
        // as a rule built out of stages does.
        shown_when: None,
    }
}
