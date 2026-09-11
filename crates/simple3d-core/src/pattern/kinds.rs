//! The turn arithmetic the stages and the grips share.
//!
//! Each fixed kind used to have a function of its own here that placed its
//! copies. They are gone: every kind is laid out as the stages that say the
//! same thing (see [`template_stages`]), so a fixed kind and the rule started
//! from it cannot drift apart, and whatever a stage learns to do the fixed
//! kinds can be started from and then do too.

use super::*;
use crate::xform::Xform;
use simple3d_geom::Vec3;

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
