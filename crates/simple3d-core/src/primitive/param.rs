//! What one parameter of a shape is: its kind, its value, and the range it
//! is allowed.

use serde::{Deserialize, Serialize};

/// What a parameter is, which is all the property editor needs to render and
/// validate a field for it.
#[derive(Clone, Copy, Debug)]
pub enum ParamKind {
    /// A length, stored in millimetres and shown in the display unit.
    Length {
        min: f64,
    },
    /// An integer count, e.g. a polygon's number of sides.
    Count {
        min: u32,
        max: u32,
    },
    /// An angle in degrees.
    Angle {
        min: f64,
        max: f64,
    },
    Bool,
    /// A radio-style choice between named alternatives, for measurements that
    /// are otherwise ambiguous (across corners vs across flats, wall thickness
    /// vs inner diameter).
    Choice {
        options: &'static [&'static str],
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "lowercase")]
pub enum ParamValue {
    Length(f64),
    Count(u32),
    Angle(f64),
    Bool(bool),
    Choice(u32),
}

impl ParamValue {
    pub fn as_f64(self) -> f64 {
        match self {
            ParamValue::Length(v) | ParamValue::Angle(v) => v,
            ParamValue::Count(v) | ParamValue::Choice(v) => v as f64,
            ParamValue::Bool(b) => b as u8 as f64,
        }
    }

    pub fn as_u32(self) -> u32 {
        match self {
            ParamValue::Count(v) | ParamValue::Choice(v) => v,
            ParamValue::Bool(b) => b as u32,
            ParamValue::Length(v) | ParamValue::Angle(v) => v.max(0.0) as u32,
        }
    }

    pub fn as_bool(self) -> bool {
        match self {
            ParamValue::Bool(b) => b,
            other => other.as_u32() != 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ParamSpec {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: ParamKind,
    pub default: ParamValue,
    /// Parameters sharing a non-zero lock group can be tied together by a lock
    /// toggle in the property editor (a sphere's three diameters, a cylinder's
    /// two). Zero means no lock.
    pub lock_group: u8,
    /// When set, this parameter is only shown while the named `Choice`
    /// parameter has the given value -- how "wall thickness *or* inner
    /// diameter" is expressed without a second primitive type.
    pub shown_when: Option<(&'static str, u32)>,
}

impl ParamSpec {
    pub(super) const fn length(key: &'static str, label: &'static str, default: f64) -> ParamSpec {
        ParamSpec {
            key,
            label,
            kind: ParamKind::Length { min: 0.0 },
            default: ParamValue::Length(default),
            lock_group: 0,
            shown_when: None,
        }
    }

    pub(super) const fn locked_length(key: &'static str, label: &'static str, default: f64, group: u8) -> ParamSpec {
        ParamSpec { lock_group: group, ..ParamSpec::length(key, label, default) }
    }

    pub(super) const fn positive_length(key: &'static str, label: &'static str, default: f64) -> ParamSpec {
        ParamSpec { kind: ParamKind::Length { min: 1e-3 }, ..ParamSpec::length(key, label, default) }
    }

    pub(super) const fn sides(default: u32) -> ParamSpec {
        ParamSpec {
            key: "sides",
            label: "Number of sides",
            kind: ParamKind::Count { min: 3, max: 128 },
            default: ParamValue::Count(default),
            lock_group: 0,
            shown_when: None,
        }
    }

    pub(super) const fn choice(key: &'static str, label: &'static str, options: &'static [&'static str]) -> ParamSpec {
        ParamSpec {
            key,
            label,
            kind: ParamKind::Choice { options },
            default: ParamValue::Choice(0),
            lock_group: 0,
            shown_when: None,
        }
    }

    pub(super) const fn when(self, param: &'static str, value: u32) -> ParamSpec {
        ParamSpec { shown_when: Some((param, value)), ..self }
    }

    /// How far round a turn a revolved shape goes. Always the same range and
    /// the same default, so a swept cylinder and a swept tube read alike.
    pub(super) const fn sweep() -> ParamSpec {
        ParamSpec {
            key: "sweep",
            label: "Sweep angle",
            kind: ParamKind::Angle { min: 1.0, max: 360.0 },
            default: ParamValue::Angle(360.0),
            lock_group: 0,
            shown_when: None,
        }
    }

    pub(super) const fn count(key: &'static str, label: &'static str, default: u32, min: u32, max: u32) -> ParamSpec {
        ParamSpec {
            key,
            label,
            kind: ParamKind::Count { min, max },
            default: ParamValue::Count(default),
            lock_group: 0,
            shown_when: None,
        }
    }
}
