//! One placed copy, and how many copies a pattern comes to.

use super::*;
use crate::primitive::{Params, ParamsExt};
use crate::xform::Xform;
use simple3d_geom::Vec3;

/// A single copy the pattern makes: where to put it, and whether it is a
/// reflection (whose winding must be flipped so its faces still point out).
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
    match axis {
        0 => r.x = angle_deg,
        1 => r.y = angle_deg,
        _ => r.z = angle_deg,
    }
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

/// The axis a radius is measured along for a turn about `axis`: the next axis
/// round, so a ring about Z lies out along X.
pub fn radial_axis(axis: usize) -> usize {
    (axis + 1) % 3
}

/// The stages a pattern is laid out by, whatever its kind.
///
/// A custom rule's own stages, or -- for any of the six fixed kinds -- the
/// stages that say exactly what that kind says. One engine lays every pattern
/// out: the fixed kinds are a name and a short form for a rule, not a second
/// piece of arithmetic that has to be kept agreeing with the first.
pub fn rule_stages(params: &Params) -> Vec<Stage> {
    match params.int("kind") {
        CUSTOM => (0..stage_count(params)).map(|index| stage(params, index)).collect(),
        kind => template_stages(params, kind),
    }
}

/// The copies a pattern makes, in its own frame, ready to be laid over the
/// mesh of its children. Always at least one -- the original -- so a pattern
/// with a count of one, or of nonsense, still shows what it holds.
pub fn instances(params: &Params) -> Vec<Instance> {
    let mut out = lay_out(&rule_stages(params));
    // A little randomness on top of whatever the rule said, so a run of planks
    // is not wallpaper (issue 79). After the cap is worked out but before it
    // bites, so which copies survive a capped pattern does not depend on it.
    noise::scatter(params, &mut out);
    // The cap is applied here rather than in each stage so nothing can forget
    // it, and by truncation rather than by refusing: the pattern still shows
    // what it makes, just not more of it than anything can draw.
    // `instance_count` says whether this bit, so the editor can tell the user.
    out.truncate(MAX_INSTANCES);
    out
}

/// The copies the rule makes by the end of stage `last`, zero-based, with no
/// scatter on them: what the tool marks in the viewport while the pointer is
/// over a stage, so "each stage repeats what the ones above it made" is
/// something to see rather than a sentence to take on trust (issue 79).
pub fn instances_through(params: &Params, last: usize) -> Vec<Instance> {
    let stages = rule_stages(params);
    let mut out = lay_out(&stages[..(last + 1).min(stages.len())]);
    out.truncate(MAX_INSTANCES);
    out
}

/// How many copies a pattern asks for and how many it will actually lay down.
/// The two differ only where [`MAX_INSTANCES`] has cut in, which is what the
/// property editor says out loud rather than silently drawing fewer.
pub fn instance_count(params: &Params) -> (usize, usize) {
    // Every stage repeats what the ones before it made, so the copies multiply
    // -- which is also exactly what a grid's three counts do.
    let wanted =
        rule_stages(params).iter().map(Stage::copies).fold(1usize, |total, copies| total.saturating_mul(copies));
    (wanted, wanted.min(MAX_INSTANCES))
}
