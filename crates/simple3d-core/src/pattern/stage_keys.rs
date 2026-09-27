//! The parameters of one stage of a custom pattern.

use super::*;
use crate::primitive::{Params, ParamsExt};

/// The parameter names one stage owns and its handle labels, in a `'static` table as [`ParamSpec`]
/// and [`Grip`] need. Labels carry the stage number ("1 Copies") because value fields are
/// remembered by label; the tool draws them without it.
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
    /// How many variations the stage has (issue 79); their own numbers are named by [`vary_key`].
    pub variations: &'static str,
    /// The old count of used fixed variation slots; read once for migration, never written.
    pub(super) legacy_varied: &'static str,
    /// The old single-variation-set fields; read once for migration, never written.
    pub(super) legacy: LegacyVary,
    /// The old mirror flag replaced by the mode; read once for migration, never written.
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

/// One stage's row of the table, every name built from the stage number, instead of eighty
/// hand-typed strings.
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

/// How many parameters of its own one stage has, besides its variations'.
pub const STAGE_KEY_COUNT: usize = 11;

/// Every key a stage owns itself (mode, numbers, variation count); variations' own are
/// [`variation_keys`].
pub fn stage_keys(stage: usize) -> Vec<&'static str> {
    let k = &STAGES[stage.min(MAX_STAGES - 1)];
    vec![k.mode, k.count, k.step[0], k.step[1], k.step[2], k.axis, k.turn, k.radius, k.growth, k.rise, k.variations]
}

/// How many stages a custom rule is using.
pub fn stage_count(params: &Params) -> usize {
    params.int("stages").clamp(1, MAX_STAGES as u32) as usize
}

/// How many variations stage `stage` is using.
pub fn variation_count(params: &Params, stage: usize) -> usize {
    params.int(STAGES[stage.min(MAX_STAGES - 1)].variations).min(MAX_VARIATIONS) as usize
}

/// The zero-based stage a key belongs to, or `None` for other keys.
pub(crate) fn stage_of(key: &str) -> Option<usize> {
    let digit = key.strip_prefix("stage")?.as_bytes().first().copied()?;
    let index = digit.checked_sub(b'1')? as usize;
    (index < MAX_STAGES).then_some(index)
}
