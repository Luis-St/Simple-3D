//! The parameters of one stage of a custom pattern.

use super::*;
use crate::primitive::{Params, ParamsExt};

/// The parameter names one stage owns, and the names its viewport handles go by.
///
/// A table rather than names built with `format!` at each use: the parameter
/// keys and the grip labels have to be `'static` to be a [`ParamSpec`] and a
/// [`Grip`], and having them in one place is what keeps the maths, the editor
/// and the handles talking about the same stage.
///
/// The numeric labels -- "1 Copies", "1.2 Shift" -- are what the field
/// answers to rather than what it reads as: a value field is remembered by its
/// label across a relayout, so four stages sharing the word "Copies" would be
/// four widgets under one name. The tool draws them without the number, because
/// the stage they sit under already says which stage they are.
pub struct StageKeys {
    pub label: &'static str,
    /// Which of the three things a stage can do (issue 79).
    pub mode: &'static str,
    pub axis: &'static str,
    pub count: &'static str,
    pub step: [&'static str; 3],
    pub turn: &'static str,
    pub radius: &'static str,
    pub growth: &'static str,
    /// How far along the axis each copy of a turning stage climbs.
    pub rise: &'static str,
    /// How many variations the stage has (issue 79). Each one's own numbers
    /// are named by [`vary_key`].
    pub variations: &'static str,
    /// How many of its four fixed variation slots a stage used, while it had
    /// four. Read once, when an older rule is migrated, and never written.
    pub(super) legacy_varied: &'static str,
    /// What a stage varied its copies by before it could vary them more than
    /// one way. Read once, when an older rule is migrated, and never written.
    pub(super) legacy: LegacyVary,
    /// The flag a stage used to carry instead of a mode. Read once, when a rule
    /// saved before the mode existed is migrated, and never written.
    pub(super) legacy_mirror: &'static str,
    pub(super) grip_spacing: &'static str,
    pub(super) grip_copies: &'static str,
    pub(super) grip_radius: &'static str,
}

/// The five numbers a stage varied its copies by while it had one of each.
pub(super) struct LegacyVary {
    pub(super) gap_growth: &'static str,
    pub(super) shift: [&'static str; 3],
    pub(super) shift_every: &'static str,
    pub(super) spin: &'static str,
    pub(super) scale: &'static str,
}

/// One stage's row of the table, every name spelled out from the stage number.
///
/// Written once rather than four times over: a score of names a stage, typed by
/// hand for each of four stages, is eighty strings that each have to say the
/// right number, and the first new number a stage learns is another four.
macro_rules! stage_keys {
    ($n:literal) => {
        StageKeys {
            label: concat!("Stage ", $n),
            mode: concat!("stage", $n, "_mode"),
            axis: concat!("stage", $n, "_axis"),
            count: concat!("stage", $n, "_count"),
            step: [concat!("stage", $n, "_step_x"), concat!("stage", $n, "_step_y"), concat!("stage", $n, "_step_z")],
            turn: concat!("stage", $n, "_turn"),
            radius: concat!("stage", $n, "_radius"),
            growth: concat!("stage", $n, "_growth"),
            rise: concat!("stage", $n, "_rise"),
            variations: concat!("stage", $n, "_variations"),
            legacy_varied: concat!("stage", $n, "_varied"),
            legacy: LegacyVary {
                gap_growth: concat!("stage", $n, "_gap_growth"),
                shift: [
                    concat!("stage", $n, "_shift_x"),
                    concat!("stage", $n, "_shift_y"),
                    concat!("stage", $n, "_shift_z"),
                ],
                shift_every: concat!("stage", $n, "_shift_every"),
                spin: concat!("stage", $n, "_spin"),
                scale: concat!("stage", $n, "_scale"),
            },
            legacy_mirror: concat!("stage", $n, "_mirror"),
            grip_spacing: concat!("Stage ", $n, " spacing"),
            grip_copies: concat!("Stage ", $n, " copies"),
            grip_radius: concat!("Stage ", $n, " radius"),
        }
    };
}

pub const STAGES: [StageKeys; MAX_STAGES] = [stage_keys!(1), stage_keys!(2), stage_keys!(3), stage_keys!(4)];

/// How many parameters of its own one stage has, beside its variations'.
pub const STAGE_KEY_COUNT: usize = 11;

/// Every parameter key a stage owns itself: what it does, its own numbers, and
/// how many variations it has. The variations' own are [`variation_keys`].
pub fn stage_keys(stage: usize) -> Vec<&'static str> {
    let k = &STAGES[stage.min(MAX_STAGES - 1)];
    vec![k.mode, k.count, k.step[0], k.step[1], k.step[2], k.axis, k.turn, k.radius, k.growth, k.rise, k.variations]
}

/// How many stages a custom rule is currently using.
pub fn stage_count(params: &Params) -> usize {
    params.int("stages").clamp(1, MAX_STAGES as u32) as usize
}

/// How many variations stage `stage` is currently using.
pub fn variation_count(params: &Params, stage: usize) -> usize {
    params.int(STAGES[stage.min(MAX_STAGES - 1)].variations).min(MAX_VARIATIONS) as usize
}

/// The stage a parameter key belongs to, zero-based -- `None` for every key
/// that is not one of a stage's own.
pub(crate) fn stage_of(key: &str) -> Option<usize> {
    let digit = key.strip_prefix("stage")?.as_bytes().first().copied()?;
    let index = digit.checked_sub(b'1')? as usize;
    (index < MAX_STAGES).then_some(index)
}
