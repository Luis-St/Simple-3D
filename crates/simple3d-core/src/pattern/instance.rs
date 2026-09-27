//! One placed copy, and how many copies a pattern comes to.

use super::*;
use crate::primitive::{Params, ParamsExt};
use crate::xform::Xform;
use simple3d_geom::Vec3;

/// One copy: its placement and whether it is a reflection, whose winding must be flipped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instance {
    pub xform: Xform,
    pub mirrored: bool,
}

impl Instance {
    pub(super) fn plain(xform: Xform) -> Instance {
        Instance { xform, mirrored: false }
    }
}

/// Euler angles that turn `angle_deg` about axis 0, 1 or 2.
pub(crate) fn rotation_about(axis: usize, angle_deg: f64) -> Vec3 {
    let mut r = Vec3::ZERO;
    r.set(axis, angle_deg);
    r
}

/// The unit vector along an axis.
pub(crate) fn unit(axis: usize) -> Vec3 {
    match axis {
        0 => Vec3::new(1.0, 0.0, 0.0),
        1 => Vec3::new(0.0, 1.0, 0.0),
        _ => Vec3::new(0.0, 0.0, 1.0),
    }
}

/// The radius axis for a turn about `axis`: the next one round, so a ring about Z lies along X.
pub fn radial_axis(axis: usize) -> usize {
    (axis + 1) % 3
}

/// The stages a pattern is laid out by: a custom rule's, or the equivalent ones for a fixed kind,
/// so one engine lays out every pattern.
pub fn rule_stages(params: &Params) -> Vec<Stage> {
    match params.int("kind") {
        CUSTOM => (0..stage_count(params)).map(|index| stage(params, index)).collect(),
        kind => template_stages(params, kind),
    }
}

/// The copies a pattern makes in its own frame; always at least the original.
pub fn instances(params: &Params) -> Vec<Instance> {
    let mut out = lay_out(&rule_stages(params));
    // Scatter on top of the rule (issue 79), before the cap, so the cap does not change which copies survive.
    noise::scatter(params, &mut out);
    // Capped here by truncation, so nothing can forget it; `instance_count` reports whether it applied.
    out.truncate(MAX_INSTANCES);
    out
}

/// The unscattered copies by the end of stage `last`, which the tool marks while hovering a stage
/// (issue 79).
pub fn instances_through(params: &Params, last: usize) -> Vec<Instance> {
    let stages = rule_stages(params);
    let mut out = lay_out(&stages[..(last + 1).min(stages.len())]);
    out.truncate(MAX_INSTANCES);
    out
}

/// How many copies a pattern asks for and how many it lays down; they differ only past
/// [`MAX_INSTANCES`], which the property editor reports.
pub fn instance_count(params: &Params) -> (usize, usize) {
    // Each stage repeats what came before, so counts multiply, as a grid's three do.
    let wanted =
        rule_stages(params).iter().map(Stage::copies).fold(1usize, |total, copies| total.saturating_mul(copies));
    (wanted, wanted.min(MAX_INSTANCES))
}
