//! Starting a custom rule from a built-in kind or a ready-made layout (issue 79).
//!
//! Every fixed kind converts to equivalent stages, so the fixed kinds serve as templates starting
//! from the numbers already on screen. The tool once opened on stage defaults, discarding the
//! user's layout.

use super::*;
use crate::primitive::{ParamValue, Params, ParamsExt};
use simple3d_geom::Vec3;

/// The stages that lay out exactly what `kind` does with its current numbers in `params`.
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

/// Put `kind`'s layout on the pattern as stages and switch to them. Unused later stages are left
/// alone; added stages get fresh numbers ([`fresh_stage`]).
pub fn use_as_template(params: &mut Params, kind: u32) {
    let stages = template_stages(params, kind);
    write_rule(params, &stages);
}

/// Start the rule from nothing (issue 79): one inert stage, the rest cleared.
pub fn clear_stages(params: &mut Params) {
    for index in 0..MAX_STAGES {
        set_stage(params, index, &Stage::run(1, Vec3::ZERO));
    }
    params.insert("stages".to_string(), ParamValue::Count(1));
    params.insert("kind".to_string(), ParamValue::Choice(CUSTOM));
}

/// The ready-made layouts, in the order the tool offers them: offset rows no fixed kind can do.
pub const PRESETS: &[&str] = &["Staggered planks", "Brick bond", "Hexagon grid"];

/// Lay `preset` out as stages sized to a shape of `size` and switch to them. The joint is a
/// twentieth of the smaller side, so copies do not touch (and weld) yet show no visible gap.
pub fn use_preset(params: &mut Params, preset: usize, size: Vec3) {
    let or_stock = |extent: f64| if extent > 1e-9 { extent } else { 20.0 };
    let (long, wide, tall) = (or_stock(size.x), or_stock(size.y), or_stock(size.z));
    let joint = long.min(wide) * 0.05;
    let (pitch, row) = (long + joint, wide + joint);
    let half = |across: f64| Stage::run(1, Vec3::ZERO).with(Variation::shift(0, across / 2.0).repeating(2));
    let stages = match preset {
        // Planks end to end along X, rows across Y, every other row offset by half a plank.
        0 => vec![
            Stage::run(5, Vec3::new(pitch, 0.0, 0.0)),
            Stage { count: 6, step: Vec3::new(0.0, row, 0.0), ..half(pitch) },
        ],
        // The same half-offset with courses stacked up Z: a wall.
        1 => vec![
            Stage::run(6, Vec3::new(pitch, 0.0, 0.0)),
            Stage { count: 8, step: Vec3::new(0.0, 0.0, tall + joint), ..half(pitch) },
        ],
        // Rows spaced by the triangle height, every other row offset by half: six equidistant neighbours.
        _ => {
            let cell = long.max(wide) + joint;
            vec![
                Stage::run(5, Vec3::new(cell, 0.0, 0.0)),
                Stage { count: 5, step: Vec3::new(0.0, cell * 3f64.sqrt() / 2.0, 0.0), ..half(cell) },
            ]
        }
    };
    write_rule(params, &stages);
    // A scatter the pattern already has is the user's and is kept.
    if Noise::of(params).wanted() {
        return;
    }
    // Otherwise the preset brings its own: planks a little (under half the joint), the aligned ones none.
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

/// Write `stages` as the pattern's rule and switch to it.
fn write_rule(params: &mut Params, stages: &[Stage]) {
    for (index, stage) in stages.iter().enumerate().take(MAX_STAGES) {
        set_stage(params, index, stage);
    }
    params.insert("stages".to_string(), ParamValue::Count(stages.len().clamp(1, MAX_STAGES) as u32));
    params.insert("kind".to_string(), ParamValue::Choice(CUSTOM));
}

fn axis_of(params: &Params, key: &str) -> usize {
    params.int(key).min(2) as usize
}
