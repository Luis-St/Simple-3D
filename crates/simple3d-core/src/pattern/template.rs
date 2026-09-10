//! Starting a custom rule from one of the built-in kinds (issue 79).
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
/// cleared: they are not in use, so they say nothing, and a rule started from a
/// grid and then grown a fourth stage finds the numbers it had before.
pub fn use_as_template(params: &mut Params, kind: u32) {
    let stages = template_stages(params, kind);
    for (index, stage) in stages.iter().enumerate() {
        set_stage(params, index, *stage);
    }
    params.insert("stages".to_string(), ParamValue::Count(stages.len().clamp(1, MAX_STAGES) as u32));
    params.insert("kind".to_string(), ParamValue::Choice(CUSTOM));
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

fn axis_of(params: &Params, key: &str) -> usize {
    params.int(key).min(2) as usize
}
