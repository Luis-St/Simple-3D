//! Every parameter each kind of pattern takes, and what a fresh one holds.

use super::*;
use crate::primitive::{ParamKind, ParamSpec, ParamValue, Params};

/// The parameter table: the fixed kinds' own numbers, then every stage's, then
/// the scatter's.
///
/// The stages are spelled out by the macro rather than by hand. Each one owns
/// seventeen parameters that differ only in the number they carry, and four of
/// them written out was sixty-eight rows in which a single mistyped digit made
/// one stage's field write another's number -- the kind of mistake nothing but
/// a test that reads every row back could catch. The defaults are the one thing
/// that differs between them, so they are what each stage is given.
macro_rules! pattern_params {
    (
        [$($head:expr),* $(,)?],
        stages [$(($n:literal, $mode:expr, $count:expr, [$sx:expr, $sy:expr, $sz:expr], $turn:expr, $radius:expr)),* $(,)?],
        [$($tail:expr),* $(,)?]
    ) => {
        &[
            $($head,)*
            $(
                does(concat!("stage", $n, "_mode"), $mode),
                axis(concat!("stage", $n, "_axis"), ("kind", CUSTOM)),
                count(concat!("stage", $n, "_count"), concat!($n, " Copies"), $count, ("kind", CUSTOM)),
                length(concat!("stage", $n, "_step_x"), concat!($n, " Step X"), $sx, ("kind", CUSTOM)),
                length(concat!("stage", $n, "_step_y"), concat!($n, " Step Y"), $sy, ("kind", CUSTOM)),
                length(concat!("stage", $n, "_step_z"), concat!($n, " Step Z"), $sz, ("kind", CUSTOM)),
                angle(concat!("stage", $n, "_turn"), concat!($n, " Turn per copy"), $turn, ("kind", CUSTOM)),
                length(concat!("stage", $n, "_radius"), concat!($n, " Radius"), $radius, ("kind", CUSTOM)),
                length(concat!("stage", $n, "_growth"), concat!($n, " Radius per copy"), 0.0, ("kind", CUSTOM)),
                length(concat!("stage", $n, "_rise"), concat!($n, " Rise per copy"), 0.0, ("kind", CUSTOM)),
                // What varies the copies rather than placing them (issue 79).
                // Every default says "no change", so a rule saved before these
                // existed lays its copies down exactly where it always did.
                length(concat!("stage", $n, "_gap_growth"), concat!($n, " Gap grows by"), 0.0, ("kind", CUSTOM)),
                length(concat!("stage", $n, "_shift_x"), concat!($n, " Shift X"), 0.0, ("kind", CUSTOM)),
                length(concat!("stage", $n, "_shift_y"), concat!($n, " Shift Y"), 0.0, ("kind", CUSTOM)),
                length(concat!("stage", $n, "_shift_z"), concat!($n, " Shift Z"), 0.0, ("kind", CUSTOM)),
                cycle(concat!("stage", $n, "_shift_every"), concat!($n, " Shift cycle"), 2, ("kind", CUSTOM)),
                angle(concat!("stage", $n, "_spin"), concat!($n, " Spin per copy"), 0.0, ("kind", CUSTOM)),
                percent(concat!("stage", $n, "_scale"), concat!($n, " Size per copy (%)"), 100, 10, 1000, ("kind", CUSTOM)),
            )*
            $($tail,)*
        ]
    };
}

/// Every parameter a pattern can carry, across all kinds. The kind choice comes
/// first and gates the rest, so the property editor shows exactly the fields the
/// current kind uses.
pub const PARAMS: &[ParamSpec] = pattern_params!(
    [
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
        // Custom: a rule the user builds themselves out of stages (issue 67).
        // Each stage repeats whatever the stages before it made, so one stage
        // is a run, two are a grid, and a run repeated round a turn is something
        // none of the fixed kinds above can say at all.
        //
        // A stage says first what it *does* -- move, turn or mirror -- and then
        // only the numbers that choice needs (issue 79). The numeric labels
        // carry the stage's number because a value field is remembered by its
        // label; the tool draws them without it, under the stage they belong to.
        count("stages", "Stages", 1, ("kind", CUSTOM)),
    ],
    stages [
        (1, 0, 3, [20.0, 0.0, 0.0], 0.0, 0.0),
        (2, 0, 2, [0.0, 20.0, 0.0], 0.0, 0.0),
        (3, 0, 2, [0.0, 0.0, 20.0], 0.0, 0.0),
        // The fourth opens as a turn, so a rule that has grown three runs long
        // offers the one thing the three before it cannot do next.
        (4, 1, 4, [0.0, 0.0, 0.0], 90.0, 40.0),
    ],
    [
        // Noise (issue 79): how far each copy may wander off where the rule
        // puts it. Not gated on a kind -- planks laid out in a run want a
        // little randomness as much as a rule built out of stages does.
        jitter("noise_x", "Jitter X"),
        jitter("noise_y", "Jitter Y"),
        jitter("noise_z", "Jitter Z"),
        // Brought into [0, 360) rather than clamped, the way the transform
        // panel's rotation is: a turn is a direction, so -30 is 330 and 400 is
        // 40, and a field that read 180 back at every larger number said
        // nothing about which of them it had been given.
        ParamSpec {
            key: "noise_turn",
            label: "Jitter turn",
            kind: ParamKind::Angle { min: 0.0, max: 360.0, wrap: true },
            default: ParamValue::Angle(0.0),
            lock_group: 0,
            shown_when: None,
        },
        // X, Y or Z, or all three at once: a plank sits askew about Z, but a
        // stone dropped on a path tilts every way.
        ParamSpec {
            key: "noise_axis",
            label: "Jitter axis",
            kind: ParamKind::Choice { options: NOISE_AXES },
            default: ParamValue::Choice(2),
            lock_group: 0,
            shown_when: None,
        },
        // How much bigger or smaller a copy may come out, either way. Capped
        // below a hundred, where a copy would shrink to nothing.
        ParamSpec {
            key: "noise_scale",
            label: "Size jitter (%)",
            kind: ParamKind::Count { min: 0, max: 90 },
            default: ParamValue::Count(0),
            lock_group: 0,
            shown_when: None,
        },
        // The seed is what makes the scatter a *choice* rather than an
        // accident: the same seed lays the same copies down every time the file
        // is opened, and the next number is a different scatter of the same
        // size.
        ParamSpec {
            key: "noise_seed",
            label: "Seed",
            kind: ParamKind::Count { min: 1, max: 9999 },
            default: ParamValue::Count(1),
            lock_group: 0,
            shown_when: None,
        },
        // The original is often the one thing that has to stay put: the first
        // plank against the wall, the part the others are measured from. Off by
        // default, because a scatter saved before this existed moved it too.
        ParamSpec {
            key: "noise_keep_first",
            label: "Leave the original in place",
            kind: ParamKind::Bool,
            default: ParamValue::Bool(false),
            lock_group: 0,
            shown_when: None,
        },
    ]
);

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
