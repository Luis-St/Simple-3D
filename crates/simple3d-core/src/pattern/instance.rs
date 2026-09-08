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

/// The copies a pattern makes, in its own frame, ready to be laid over the
/// mesh of its children. Always at least one -- the original -- so a pattern
/// with a count of one, or of nonsense, still shows what it holds.
pub fn instances(params: &Params) -> Vec<Instance> {
    let mut out = match params.int("kind") {
        GRID => grid(params),
        CIRCULAR => circular(params),
        MIRROR => mirror(params),
        HELIX => helix(params),
        SPIRAL => spiral(params),
        CUSTOM => custom(params),
        _ => linear(params),
    };
    // The cap is applied here rather than in each kind so no kind can forget it,
    // and by truncation rather than by refusing: the pattern still shows what it
    // makes, just not more of it than anything can draw. `instance_count` says
    // whether this bit, so the editor can tell the user.
    out.truncate(MAX_INSTANCES);
    out
}

/// How many copies a pattern asks for and how many it will actually lay down.
/// The two differ only where [`MAX_INSTANCES`] has cut in, which is what the
/// property editor says out loud rather than silently drawing fewer.
pub fn instance_count(params: &Params) -> (usize, usize) {
    let wanted = match params.int("kind") {
        GRID => {
            let (nx, ny, nz) = (
                params.int("grid_x").max(1) as usize,
                params.int("grid_y").max(1) as usize,
                params.int("grid_z").max(1) as usize,
            );
            nx.saturating_mul(ny).saturating_mul(nz)
        }
        CIRCULAR => params.int("circ_count").max(1) as usize,
        MIRROR => 2,
        HELIX => params.int("helix_count").max(1) as usize,
        SPIRAL => params.int("spiral_count").max(1) as usize,
        // Every stage repeats what the ones before it made, so the copies
        // multiply exactly as a grid's three counts do.
        CUSTOM => (0..stage_count(params))
            .map(|s| stage(params, s).copies())
            .fold(1usize, |total, copies| total.saturating_mul(copies)),
        _ => params.int("count").max(1) as usize,
    };
    (wanted, wanted.min(MAX_INSTANCES))
}
