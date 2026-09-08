//! Where each kind of pattern puts its copies.

use super::*;
use crate::primitive::{Params, ParamsExt};
use crate::xform::Xform;
use simple3d_geom::Vec3;

pub(crate) fn linear(params: &Params) -> Vec<Instance> {
    let count = params.int("count").max(1);
    let step = Vec3::new(params.num("step_x"), params.num("step_y"), params.num("step_z"));
    (0..count).map(|i| Instance::plain(Xform::from_translation(step * i as f64))).collect()
}

pub(crate) fn grid(params: &Params) -> Vec<Instance> {
    let (nx, ny, nz) = (params.int("grid_x").max(1), params.int("grid_y").max(1), params.int("grid_z").max(1));
    let step = Vec3::new(params.num("grid_step_x"), params.num("grid_step_y"), params.num("grid_step_z"));
    // Stop at the cap *while building* rather than by truncating afterwards: the
    // three counts multiply, so a full 512 x 512 x 512 would have to be
    // allocated -- 14 GB of transforms -- before anything could trim it.
    let mut out = Vec::new();
    'fill: for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                if out.len() >= MAX_INSTANCES {
                    break 'fill;
                }
                let offset = Vec3::new(step.x * i as f64, step.y * j as f64, step.z * k as f64);
                out.push(Instance::plain(Xform::from_translation(offset)));
            }
        }
    }
    out
}

/// The angle between copies for a turn: a full turn divides by the count so the
/// first and last copy do not land on each other, a partial one divides by the
/// gaps so both ends are placed.
pub(crate) fn step_angle(span: f64, count: u32) -> f64 {
    if count <= 1 {
        return 0.0;
    }
    if (span.abs() - 360.0).abs() < 1e-6 {
        span / count as f64
    } else {
        span / (count - 1) as f64
    }
}

/// A copy at radius `r` and angle `a` about `ax`, optionally lifted along the
/// axis: place it out at the radius, turn it about the axis, then lift it.
pub(crate) fn turned(ax: usize, r: f64, angle_deg: f64, lift: f64) -> Xform {
    let place = Xform::from_translation(unit(radial_axis(ax)) * r);
    let turn = Xform::from_pos_rot(Vec3::ZERO, rotation_about(ax, angle_deg));
    let rise = Xform::from_translation(unit(ax) * lift);
    rise.compose(&turn.compose(&place))
}

pub(crate) fn circular(params: &Params) -> Vec<Instance> {
    let count = params.int("circ_count").max(1);
    let span = params.num("circ_span");
    let radius = params.num("circ_radius");
    let ax = params.int("circ_axis").min(2) as usize;
    let step = step_angle(span, count);
    (0..count).map(|i| Instance::plain(turned(ax, radius, step * i as f64, 0.0))).collect()
}

pub(crate) fn mirror(params: &Params) -> Vec<Instance> {
    let ax = params.int("mirror_axis").min(2) as usize;
    // A reflection across the plane through the origin whose normal is the axis:
    // the identity with that axis negated. Its winding is flipped when it is laid
    // down so the reflected faces still point outward.
    let mut m = Xform::IDENTITY;
    m.m[ax][ax] = -1.0;
    vec![Instance::plain(Xform::IDENTITY), Instance { xform: m, mirrored: true }]
}

pub(crate) fn helix(params: &Params) -> Vec<Instance> {
    let count = params.int("helix_count").max(1);
    let step = params.num("helix_angle");
    let rise = params.num("helix_rise");
    let radius = params.num("helix_radius");
    let ax = params.int("helix_axis").min(2) as usize;
    (0..count).map(|i| Instance::plain(turned(ax, radius, step * i as f64, rise * i as f64))).collect()
}

pub(crate) fn spiral(params: &Params) -> Vec<Instance> {
    let count = params.int("spiral_count").max(1);
    let step = params.num("spiral_angle");
    let r0 = params.num("spiral_radius");
    let dr = params.num("spiral_growth");
    let rise = params.num("spiral_rise");
    let ax = params.int("spiral_axis").min(2) as usize;
    (0..count).map(|i| Instance::plain(turned(ax, r0 + dr * i as f64, step * i as f64, rise * i as f64))).collect()
}
