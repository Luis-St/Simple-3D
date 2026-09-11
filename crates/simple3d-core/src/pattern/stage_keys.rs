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
/// The numeric labels -- "1 Copies", "1.2 Shift X" -- are what the field
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
    /// How many of the stage's variations are in use (issue 79).
    pub varied: &'static str,
    /// Each variation's own names, in the order the stage applies them.
    pub vary: [VaryKeys; MAX_VARIATIONS],
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

/// The names one variation of a stage owns (issue 79).
///
/// Every variation carries the numbers of all four kinds, and its kind says
/// which of them it reads. That is what keeps a variation a fixed handful of
/// ordinary parameters -- the property editor, the project file, the clipboard
/// and undo all need a name for each number -- and it is also what lets a
/// variation change its kind without losing what was typed into it.
pub struct VaryKeys {
    /// What it changes: a [`Vary`].
    pub what: &'static str,
    /// Whether its amount builds up copy by copy or comes round on a cycle.
    pub steps: &'static str,
    /// How many copies the cycle runs over.
    pub every: &'static str,
    pub offset: [&'static str; 3],
    pub angle: &'static str,
    pub axis: &'static str,
    /// How big each step makes a copy, in percent.
    pub size: &'static str,
    pub gap: &'static str,
}

impl VaryKeys {
    /// Every one of them.
    pub fn all(&self) -> [&'static str; VARY_KEY_COUNT] {
        [
            self.what,
            self.steps,
            self.every,
            self.offset[0],
            self.offset[1],
            self.offset[2],
            self.angle,
            self.axis,
            self.size,
            self.gap,
        ]
    }
}

/// The five numbers a stage varied its copies by while it had one of each.
pub(super) struct LegacyVary {
    pub(super) gap_growth: &'static str,
    pub(super) shift: [&'static str; 3],
    pub(super) shift_every: &'static str,
    pub(super) spin: &'static str,
    pub(super) scale: &'static str,
}

/// One variation's row of the table, spelled out from the stage and slot
/// numbers.
macro_rules! vary_keys {
    ($n:literal, $m:literal) => {
        VaryKeys {
            what: concat!("stage", $n, "_vary", $m, "_what"),
            steps: concat!("stage", $n, "_vary", $m, "_steps"),
            every: concat!("stage", $n, "_vary", $m, "_every"),
            offset: [
                concat!("stage", $n, "_vary", $m, "_x"),
                concat!("stage", $n, "_vary", $m, "_y"),
                concat!("stage", $n, "_vary", $m, "_z"),
            ],
            angle: concat!("stage", $n, "_vary", $m, "_angle"),
            axis: concat!("stage", $n, "_vary", $m, "_axis"),
            size: concat!("stage", $n, "_vary", $m, "_size"),
            gap: concat!("stage", $n, "_vary", $m, "_gap"),
        }
    };
}

/// One stage's row of the table, every name spelled out from the stage number.
///
/// Written once rather than four times over: fifty-one names a stage, typed by
/// hand for each of four stages, is two hundred strings that each have to say
/// the right number, and the first new number a stage learns is another four.
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
            varied: concat!("stage", $n, "_varied"),
            vary: [vary_keys!($n, 1), vary_keys!($n, 2), vary_keys!($n, 3), vary_keys!($n, 4)],
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

/// How many parameters one variation owns.
pub const VARY_KEY_COUNT: usize = 10;

/// How many parameters one stage owns: its own eleven, and its variations'.
pub const STAGE_KEY_COUNT: usize = 11 + MAX_VARIATIONS * VARY_KEY_COUNT;

/// Every parameter key a stage owns: what the stage does and its own numbers
/// first, then the ones that vary its copies.
pub fn stage_keys(stage: usize) -> Vec<&'static str> {
    let k = &STAGES[stage.min(MAX_STAGES - 1)];
    let mut keys =
        vec![k.mode, k.count, k.step[0], k.step[1], k.step[2], k.axis, k.turn, k.radius, k.growth, k.rise, k.varied];
    keys.extend(vary_keys(stage));
    keys
}

/// The keys of every variation slot a stage has, in slot order (issue 79).
pub fn vary_keys(stage: usize) -> Vec<&'static str> {
    STAGES[stage.min(MAX_STAGES - 1)].vary.iter().flat_map(VaryKeys::all).collect()
}

/// How many stages a custom rule is currently using.
pub fn stage_count(params: &Params) -> usize {
    params.int("stages").clamp(1, MAX_STAGES as u32) as usize
}

/// How many variations stage `stage` is currently using.
pub fn variation_count(params: &Params, stage: usize) -> usize {
    params.int(STAGES[stage.min(MAX_STAGES - 1)].varied).min(MAX_VARIATIONS as u32) as usize
}

/// The stage a parameter key belongs to, zero-based -- `None` for every key
/// that is not one of a stage's own.
pub(crate) fn stage_of(key: &str) -> Option<usize> {
    let digit = key.strip_prefix("stage")?.as_bytes().first().copied()?;
    let index = digit.checked_sub(b'1')? as usize;
    (index < MAX_STAGES).then_some(index)
}
