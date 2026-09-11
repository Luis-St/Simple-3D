//! Starting a custom rule from one of the built-in kinds, or from one of the
//! layouts that ship ready made (issue 79).
//!
//! Every fixed kind is one or two stages spelled out -- that is the check that
//! the stage model is the rule underneath them rather than a seventh special
//! case -- so any of them can be written back into the stages that say the same
//! thing. That turns "Linear", "Grid" and the rest into templates: lay a
//! pattern out with a kind that nearly does it, open the tool, and start from
//! the numbers already on screen rather than from a stock 20 mm step.
//!
//! Which is also what makes a custom rule reachable at all. The tool used to
//! open on the stage defaults whatever the pattern was already doing, so the
//! first thing it did was throw away the layout the user had just made.

use super::*;
use crate::primitive::{ParamValue, Params, ParamsExt};
use simple3d_geom::Vec3;

/// The stages that lay out exactly what `kind` lays out, given the numbers that
/// kind is currently holding in `params`.
pub fn template_stages(params: &Params, kind: u32) -> Vec<Stage> {
    match kind {
        GRID => {
            let step = Vec3::new(params.num("grid_step_x"), params.num("grid_step_y"), params.num("grid_step_z"));
            vec![
                Stage::run(params.int("grid_x").max(1), Vec3::new(step.x, 0.0, 0.0)),
                Stage::run(params.int("grid_y").max(1), Vec3::new(0.0, step.y, 0.0)),
                Stage::run(params.int("grid_z").max(1), Vec3::new(0.0, 0.0, step.z)),
            ]
        }
        CIRCULAR => {
            let count = params.int("circ_count").max(1);
            let turn = step_angle(params.num("circ_span"), count);
            vec![Stage::turning(count, turn, params.num("circ_radius"), 0.0, 0.0, axis_of(params, "circ_axis"))]
        }
        MIRROR => vec![Stage::mirrored(axis_of(params, "mirror_axis"))],
        HELIX => vec![Stage::turning(
            params.int("helix_count").max(1),
            params.num("helix_angle"),
            params.num("helix_radius"),
            0.0,
            params.num("helix_rise"),
            axis_of(params, "helix_axis"),
        )],
        SPIRAL => vec![Stage::turning(
            params.int("spiral_count").max(1),
            params.num("spiral_angle"),
            params.num("spiral_radius"),
            params.num("spiral_growth"),
            params.num("spiral_rise"),
            axis_of(params, "spiral_axis"),
        )],
        _ => vec![Stage::run(
            params.int("count").max(1),
            Vec3::new(params.num("step_x"), params.num("step_y"), params.num("step_z")),
        )],
    }
}

/// Put `kind`'s layout on the pattern as stages, and switch it to them.
///
/// The stages past the ones the template needs are left alone rather than
/// cleared: they are not in use, so they say nothing, and a stage that is added
/// afterwards is given fresh numbers of its own (see [`fresh_stage`]).
pub fn use_as_template(params: &mut Params, kind: u32) {
    let stages = template_stages(params, kind);
    write_rule(params, &stages);
}

/// Start the rule from nothing: one stage that does not move, turn or mirror,
/// and the other three cleared behind it.
///
/// The seventh answer to "what do I start from" (issue 79). The six kinds are
/// the layouts worth starting from; this is for the rule that is none of them,
/// and it is a blank sheet rather than the numbers whichever kind the pattern
/// happened to be holding -- which is the whole difference between it and the
/// six.
pub fn clear_stages(params: &mut Params) {
    for index in 0..MAX_STAGES {
        set_stage(params, index, Stage::run(1, Vec3::ZERO));
    }
    params.insert("stages".to_string(), ParamValue::Count(1));
    params.insert("kind".to_string(), ParamValue::Choice(CUSTOM));
}

/// The layouts that ship ready made, in the order the tool offers them.
///
/// Each is something none of the six fixed kinds can say and the issue that
/// asked for them named: rows offset against each other. They are rules like
/// any other once started from -- the numbers are ordinary stage numbers, and
/// the shift that offsets every other row is on screen to be changed.
pub const PRESETS: &[&str] = &["Staggered planks", "Brick bond", "Hexagon grid"];

/// Lay `preset` out as stages, sized to a shape of `size`, and switch the
/// pattern to them.
///
/// Sized rather than stock, for the reason a new pattern's step is: a deck of
/// 1 m planks and a sheet of 10 mm tiles want the same rule at a hundred times
/// the spacing, and a preset that laid either at the other's would be a set of
/// numbers to retype rather than a starting point. The joint between two
/// copies is a twentieth of the shape's smaller side, so the copies stand clear
/// of each other -- copies that touch are welded into one body -- without the
/// gap being something anyone would see as a gap.
pub fn use_preset(params: &mut Params, preset: usize, size: Vec3) {
    let or_stock = |extent: f64| if extent > 1e-9 { extent } else { 20.0 };
    let (long, wide, tall) = (or_stock(size.x), or_stock(size.y), or_stock(size.z));
    let joint = long.min(wide) * 0.05;
    let (pitch, row) = (long + joint, wide + joint);
    let half =
        |across: f64| Stage { shift: Vec3::new(across / 2.0, 0.0, 0.0), shift_every: 2, ..Stage::run(1, Vec3::ZERO) };
    let stages = match preset {
        // Planks end to end along X, rows of them across Y, every other row
        // moved on by half a plank so no two joints line up.
        0 => vec![
            Stage::run(5, Vec3::new(pitch, 0.0, 0.0)),
            Stage { count: 6, step: Vec3::new(0.0, row, 0.0), ..half(pitch) },
        ],
        // The same half-offset, with the courses stacked up Z: a wall.
        1 => vec![
            Stage::run(6, Vec3::new(pitch, 0.0, 0.0)),
            Stage { count: 8, step: Vec3::new(0.0, 0.0, tall + joint), ..half(pitch) },
        ],
        // Rows as far apart as the height of the triangle between three
        // neighbours, every other row moved on by half a cell: each copy then
        // has six neighbours all the same distance off.
        _ => {
            let cell = long.max(wide) + joint;
            vec![
                Stage::run(5, Vec3::new(cell, 0.0, 0.0)),
                Stage { count: 5, step: Vec3::new(0.0, cell * 3f64.sqrt() / 2.0, 0.0), ..half(cell) },
            ]
        }
    };
    write_rule(params, &stages);
    // The scatter is part of what a preset is. Planks come with a little --
    // less than half the joint either way, so they wander without touching --
    // and the two whose whole point is that every copy lines up come with none.
    for key in noise_keys() {
        if let Some(spec) = PARAMS.iter().find(|p| p.key == *key) {
            params.insert((*key).to_string(), spec.default);
        }
    }
    if preset == 0 {
        params.insert("noise_x".to_string(), ParamValue::Length(joint * 0.4));
        params.insert("noise_y".to_string(), ParamValue::Length(joint * 0.4));
    }
}

/// Write `stages` as the rule the pattern uses, and switch the pattern to it.
fn write_rule(params: &mut Params, stages: &[Stage]) {
    for (index, stage) in stages.iter().enumerate().take(MAX_STAGES) {
        set_stage(params, index, *stage);
    }
    params.insert("stages".to_string(), ParamValue::Count(stages.len().clamp(1, MAX_STAGES) as u32));
    params.insert("kind".to_string(), ParamValue::Choice(CUSTOM));
}

fn axis_of(params: &Params, key: &str) -> usize {
    params.int(key).min(2) as usize
}
