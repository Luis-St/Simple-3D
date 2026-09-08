//! Every parameter each kind of pattern takes, and what a fresh one holds.

use super::*;
use crate::primitive::{ParamKind, ParamSpec, ParamValue, Params};

/// Every parameter a pattern can carry, across all kinds. The kind choice comes
/// first and gates the rest, so the property editor shows exactly the fields the
/// current kind uses.
pub const PARAMS: &[ParamSpec] = &[
    ParamSpec {
        key: "kind",
        label: "Kind",
        kind: ParamKind::Choice { options: KINDS },
        default: ParamValue::Choice(LINEAR),
        lock_group: 0,
        shown_when: None,
    },
    // Linear: a run of copies along a fixed step.
    count("count", "Copies", 3, ("kind", LINEAR)),
    length("step_x", "Step X", 20.0, ("kind", LINEAR)),
    length("step_y", "Step Y", 0.0, ("kind", LINEAR)),
    length("step_z", "Step Z", 0.0, ("kind", LINEAR)),
    // Grid: a lattice, one count and one step per axis.
    count("grid_x", "Columns (X)", 3, ("kind", GRID)),
    count("grid_y", "Rows (Y)", 2, ("kind", GRID)),
    count("grid_z", "Layers (Z)", 1, ("kind", GRID)),
    length("grid_step_x", "Step X", 20.0, ("kind", GRID)),
    length("grid_step_y", "Step Y", 20.0, ("kind", GRID)),
    length("grid_step_z", "Step Z", 20.0, ("kind", GRID)),
    // Circular: copies evenly round a turn.
    count("circ_count", "Copies", 6, ("kind", CIRCULAR)),
    angle("circ_span", "Span", 360.0, ("kind", CIRCULAR)),
    length("circ_radius", "Radius", 0.0, ("kind", CIRCULAR)),
    axis("circ_axis", ("kind", CIRCULAR)),
    // Mirror: the original and its reflection across a plane.
    axis("mirror_axis", ("kind", MIRROR)),
    // Helix: turn and rise together.
    count("helix_count", "Copies", 8, ("kind", HELIX)),
    angle("helix_angle", "Angle per copy", 45.0, ("kind", HELIX)),
    length("helix_rise", "Rise per copy", 5.0, ("kind", HELIX)),
    length("helix_radius", "Radius", 20.0, ("kind", HELIX)),
    axis("helix_axis", ("kind", HELIX)),
    // Spiral: turn while the radius grows.
    count("spiral_count", "Copies", 12, ("kind", SPIRAL)),
    angle("spiral_angle", "Angle per copy", 30.0, ("kind", SPIRAL)),
    length("spiral_radius", "Start radius", 10.0, ("kind", SPIRAL)),
    length("spiral_growth", "Radius per copy", 5.0, ("kind", SPIRAL)),
    length("spiral_rise", "Rise per copy", 0.0, ("kind", SPIRAL)),
    axis("spiral_axis", ("kind", SPIRAL)),
    // Custom: a rule the user builds themselves out of stages (issue 67). Each
    // stage repeats whatever the stages before it made, so one stage is a run,
    // two are a grid, and a run repeated round a turn is something none of the
    // fixed kinds above can say at all.
    count("stages", "Stages", 1, ("kind", CUSTOM)),
    flag("stage1_mirror", "1 Mirror", ("kind", CUSTOM)),
    axis("stage1_axis", ("kind", CUSTOM)),
    count("stage1_count", "1 Copies", 3, ("kind", CUSTOM)),
    length("stage1_step_x", "1 Step X", 20.0, ("kind", CUSTOM)),
    length("stage1_step_y", "1 Step Y", 0.0, ("kind", CUSTOM)),
    length("stage1_step_z", "1 Step Z", 0.0, ("kind", CUSTOM)),
    angle("stage1_turn", "1 Turn per copy", 0.0, ("kind", CUSTOM)),
    length("stage1_radius", "1 Radius", 0.0, ("kind", CUSTOM)),
    length("stage1_growth", "1 Radius per copy", 0.0, ("kind", CUSTOM)),
    flag("stage2_mirror", "2 Mirror", ("kind", CUSTOM)),
    axis("stage2_axis", ("kind", CUSTOM)),
    count("stage2_count", "2 Copies", 2, ("kind", CUSTOM)),
    length("stage2_step_x", "2 Step X", 0.0, ("kind", CUSTOM)),
    length("stage2_step_y", "2 Step Y", 20.0, ("kind", CUSTOM)),
    length("stage2_step_z", "2 Step Z", 0.0, ("kind", CUSTOM)),
    angle("stage2_turn", "2 Turn per copy", 0.0, ("kind", CUSTOM)),
    length("stage2_radius", "2 Radius", 0.0, ("kind", CUSTOM)),
    length("stage2_growth", "2 Radius per copy", 0.0, ("kind", CUSTOM)),
    flag("stage3_mirror", "3 Mirror", ("kind", CUSTOM)),
    axis("stage3_axis", ("kind", CUSTOM)),
    count("stage3_count", "3 Copies", 2, ("kind", CUSTOM)),
    length("stage3_step_x", "3 Step X", 0.0, ("kind", CUSTOM)),
    length("stage3_step_y", "3 Step Y", 0.0, ("kind", CUSTOM)),
    length("stage3_step_z", "3 Step Z", 20.0, ("kind", CUSTOM)),
    angle("stage3_turn", "3 Turn per copy", 0.0, ("kind", CUSTOM)),
    length("stage3_radius", "3 Radius", 0.0, ("kind", CUSTOM)),
    length("stage3_growth", "3 Radius per copy", 0.0, ("kind", CUSTOM)),
    flag("stage4_mirror", "4 Mirror", ("kind", CUSTOM)),
    axis("stage4_axis", ("kind", CUSTOM)),
    count("stage4_count", "4 Copies", 4, ("kind", CUSTOM)),
    length("stage4_step_x", "4 Step X", 0.0, ("kind", CUSTOM)),
    length("stage4_step_y", "4 Step Y", 0.0, ("kind", CUSTOM)),
    length("stage4_step_z", "4 Step Z", 0.0, ("kind", CUSTOM)),
    angle("stage4_turn", "4 Turn per copy", 90.0, ("kind", CUSTOM)),
    length("stage4_radius", "4 Radius", 40.0, ("kind", CUSTOM)),
    length("stage4_growth", "4 Radius per copy", 0.0, ("kind", CUSTOM)),
];

/// The most copies one pattern will ever lay down.
///
/// Each count is clamped to 512 on its own, but a grid *multiplies* three of
/// them: 512 on every axis is 134 million copies, which is tens of gigabytes of
/// transforms before a single triangle is placed. The per-axis clamp cannot see
/// that, so the total is capped here as well -- generously enough for any run a
/// person lays out by hand, and low enough that a mistyped grid is a redrawn
/// preview rather than an out-of-memory kill.
pub const MAX_INSTANCES: usize = 4096;

/// The most stages one custom kind is built from.
///
/// Four is what a rule anyone builds by hand actually needs -- three axes and a
/// turn, which is already more than any of the fixed kinds says -- and a fixed
/// number is what lets a stage's numbers be ordinary parameters with names of
/// their own. That is not a detail: it is what makes the property editor render
/// them, the project file carry them, the clipboard copy them and undo cover
/// them, none of which needed a line of code here.
pub const MAX_STAGES: usize = 4;

/// A fresh pattern's parameters.
pub fn default_params() -> Params {
    PARAMS.iter().map(|p| (p.key.to_string(), p.default)).collect()
}
