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
/// The numeric labels -- "1 Copies", "3 Radius per copy" -- are what the field
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
    /// How much wider each gap of a run is than the one before it (issue 79).
    pub gap_growth: &'static str,
    /// The offset a copy is moved by for each place it sits into its cycle.
    pub shift: [&'static str; 3],
    /// How many copies the shift runs over before it starts again.
    pub shift_every: &'static str,
    /// How far each copy turns about its own origin, per copy.
    pub spin: &'static str,
    /// How big each copy is next to the one before it, in percent.
    pub scale: &'static str,
    /// The flag a stage used to carry instead of a mode. Read once, when a rule
    /// saved before the mode existed is migrated, and never written.
    pub(super) legacy_mirror: &'static str,
    pub(super) grip_spacing: &'static str,
    pub(super) grip_copies: &'static str,
    pub(super) grip_radius: &'static str,
}

/// One stage's row of the table, every name spelled out from the stage number.
///
/// Written once rather than four times over: seventeen names a stage, typed by
/// hand for each of four stages, is sixty-eight strings that each have to say
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
            gap_growth: concat!("stage", $n, "_gap_growth"),
            shift: [
                concat!("stage", $n, "_shift_x"),
                concat!("stage", $n, "_shift_y"),
                concat!("stage", $n, "_shift_z"),
            ],
            shift_every: concat!("stage", $n, "_shift_every"),
            spin: concat!("stage", $n, "_spin"),
            scale: concat!("stage", $n, "_scale"),
            legacy_mirror: concat!("stage", $n, "_mirror"),
            grip_spacing: concat!("Stage ", $n, " spacing"),
            grip_copies: concat!("Stage ", $n, " copies"),
            grip_radius: concat!("Stage ", $n, " radius"),
        }
    };
}

pub const STAGES: [StageKeys; MAX_STAGES] = [stage_keys!(1), stage_keys!(2), stage_keys!(3), stage_keys!(4)];

/// How many parameters one stage owns.
pub const STAGE_KEY_COUNT: usize = 17;

/// Every parameter key a stage owns, in the order the editor shows them: what
/// the stage does and its own numbers first, then the ones that vary its copies.
pub fn stage_keys(stage: usize) -> Vec<&'static str> {
    let k = &STAGES[stage.min(MAX_STAGES - 1)];
    let mut keys = vec![k.mode, k.count, k.step[0], k.step[1], k.step[2], k.axis, k.turn, k.radius, k.growth, k.rise];
    // The axis is in both lists, and a key owned twice would be saved twice.
    keys.extend(vary_keys(stage).into_iter().filter(|key| *key != k.axis));
    keys
}

/// The keys that vary a stage's copies from one to the next rather than
/// placing them (issue 79), in the order the tool's "Vary" section shows them.
///
/// The axis is here as well as among the stage's own numbers: a run has no axis
/// of its own, but a run whose copies spin has to say what they spin about. It
/// is drawn in whichever of the two places its mode shows it.
pub fn vary_keys(stage: usize) -> Vec<&'static str> {
    let k = &STAGES[stage.min(MAX_STAGES - 1)];
    vec![k.gap_growth, k.shift[0], k.shift[1], k.shift[2], k.shift_every, k.spin, k.axis, k.scale]
}

/// How many stages a custom rule is currently using.
pub fn stage_count(params: &Params) -> usize {
    params.int("stages").clamp(1, MAX_STAGES as u32) as usize
}

/// The stage a parameter key belongs to, zero-based -- `None` for every key
/// that is not one of a stage's own.
pub(crate) fn stage_of(key: &str) -> Option<usize> {
    let digit = key.strip_prefix("stage")?.as_bytes().first().copied()?;
    let index = digit.checked_sub(b'1')? as usize;
    (index < MAX_STAGES).then_some(index)
}
