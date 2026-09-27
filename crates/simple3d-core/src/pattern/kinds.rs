//! Turn arithmetic shared by stages and grips. Fixed kinds are laid out as stages
//! ([`template_stages`]), so they cannot drift from rules started from them.

use super::*;
use crate::xform::Xform;
use simple3d_geom::Vec3;

/// Angle between copies: a full turn divides by the count so ends do not coincide, a partial one by
/// the gaps so both ends are placed.
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

/// A copy at radius `r` and angle about `ax`, optionally lifted along the axis.
pub(crate) fn turned(ax: usize, r: f64, angle_deg: f64, lift: f64) -> Xform {
    let place = Xform::from_translation(unit(radial_axis(ax)) * r);
    let turn = Xform::from_pos_rot(Vec3::ZERO, rotation_about(ax, angle_deg));
    let rise = Xform::from_translation(unit(ax) * lift);
    rise.compose(&turn.compose(&place))
}
