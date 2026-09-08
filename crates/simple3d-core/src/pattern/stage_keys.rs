//! The parameters of one stage of a custom pattern.

use super::*;
use crate::primitive::{Params, ParamsExt};

/// The parameter names one stage owns, and the names its viewport handles go by.
///
/// A table rather than names built with `format!` at each use: the parameter
/// keys and the grip labels have to be `'static` to be a [`ParamSpec`] and a
/// [`Grip`], and having them in one place is what keeps the maths, the editor
/// and the handles talking about the same stage.
pub struct StageKeys {
    pub label: &'static str,
    pub mirror: &'static str,
    pub axis: &'static str,
    pub count: &'static str,
    pub step: [&'static str; 3],
    pub turn: &'static str,
    pub radius: &'static str,
    pub growth: &'static str,
    pub(super) grip_spacing: &'static str,
    pub(super) grip_copies: &'static str,
    pub(super) grip_radius: &'static str,
}

pub const STAGES: [StageKeys; MAX_STAGES] = [
    StageKeys {
        label: "Stage 1",
        mirror: "stage1_mirror",
        axis: "stage1_axis",
        count: "stage1_count",
        step: ["stage1_step_x", "stage1_step_y", "stage1_step_z"],
        turn: "stage1_turn",
        radius: "stage1_radius",
        growth: "stage1_growth",
        grip_spacing: "Stage 1 spacing",
        grip_copies: "Stage 1 copies",
        grip_radius: "Stage 1 radius",
    },
    StageKeys {
        label: "Stage 2",
        mirror: "stage2_mirror",
        axis: "stage2_axis",
        count: "stage2_count",
        step: ["stage2_step_x", "stage2_step_y", "stage2_step_z"],
        turn: "stage2_turn",
        radius: "stage2_radius",
        growth: "stage2_growth",
        grip_spacing: "Stage 2 spacing",
        grip_copies: "Stage 2 copies",
        grip_radius: "Stage 2 radius",
    },
    StageKeys {
        label: "Stage 3",
        mirror: "stage3_mirror",
        axis: "stage3_axis",
        count: "stage3_count",
        step: ["stage3_step_x", "stage3_step_y", "stage3_step_z"],
        turn: "stage3_turn",
        radius: "stage3_radius",
        growth: "stage3_growth",
        grip_spacing: "Stage 3 spacing",
        grip_copies: "Stage 3 copies",
        grip_radius: "Stage 3 radius",
    },
    StageKeys {
        label: "Stage 4",
        mirror: "stage4_mirror",
        axis: "stage4_axis",
        count: "stage4_count",
        step: ["stage4_step_x", "stage4_step_y", "stage4_step_z"],
        turn: "stage4_turn",
        radius: "stage4_radius",
        growth: "stage4_growth",
        grip_spacing: "Stage 4 spacing",
        grip_copies: "Stage 4 copies",
        grip_radius: "Stage 4 radius",
    },
];

/// Every parameter key a stage owns, in the order the editor shows them.
pub fn stage_keys(stage: usize) -> Vec<&'static str> {
    let k = &STAGES[stage.min(MAX_STAGES - 1)];
    vec![k.mirror, k.axis, k.count, k.step[0], k.step[1], k.step[2], k.turn, k.radius, k.growth]
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
