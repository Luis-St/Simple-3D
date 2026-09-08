//! The handles a pattern is dragged by, and what dragging one changes.

use super::*;
use crate::primitive::{ParamValue, Params, ParamsExt};
use simple3d_geom::Vec3;

/// One handle a pattern offers for laying itself out by eye.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grip {
    /// What it drives, for the tooltip and for the undo step it records. Unique
    /// within one kind, which is what identifies a grip across the frames of a
    /// drag -- an index would shift under the drag itself, since adding a copy
    /// can add a grip.
    pub label: &'static str,
    /// Where the grip sits, in the pattern's own frame.
    pub at: Vec3,
    /// The line it slides along: a point on that line and a unit direction. For
    /// an [`Drive::Angle`] grip the line is the axis it turns about, `from` is
    /// the centre of the turn, and `radius` is how far out the grip rides.
    pub from: Vec3,
    pub dir: Vec3,
    pub radius: f64,
    pub drive: Drive,
}

impl Grip {
    pub(super) fn slide(label: &'static str, at: Vec3, dir: Vec3, drive: Drive) -> Grip {
        Grip { label, at, from: Vec3::ZERO, dir, radius: 0.0, drive }
    }
}

pub(crate) const LINEAR_RUN: &[&str] = &["step_x", "step_y", "step_z"];

/// A length that may not go below zero: a radius or the length of a run, both
/// of which mean nothing negative.
pub(crate) const POSITIVE: f64 = 0.0;

/// A length that may go either way: a step, a rise or a growth, each of which
/// lays the copies out backwards or downwards when it is negative.
pub(crate) const EITHER_WAY: f64 = f64::NEG_INFINITY;

/// The grips a pattern offers, in its own frame.
///
/// A grip whose home is the pattern's own origin is dropped: that is where the
/// move manipulator already sits, and two handles on one point cannot both be
/// grabbed. It is also where a run of no length puts everything, and a run with
/// no length has nothing to take hold of -- those numbers are typed once and
/// then dragged.
pub fn grips(params: &Params) -> Vec<Grip> {
    let mut out = match params.int("kind") {
        GRID => grid_grips(params),
        CIRCULAR => circular_grips(params),
        // A mirror is a plane and two copies. There is no distance and no count
        // to lay out, so it offers nothing to drag.
        MIRROR => Vec::new(),
        HELIX => helix_grips(params),
        SPIRAL => spiral_grips(params),
        CUSTOM => custom_grips(params),
        _ => linear_grips(params),
    };
    out.retain(|g| g.at.length() > 1e-6);
    out
}

/// The grip with this label, for a drag that started on it a frame ago.
pub fn grip(params: &Params, label: &str) -> Option<Grip> {
    grips(params).into_iter().find(|g| g.label == label)
}

/// Write what a grip was dragged to.
///
/// `value` is a distance along the grip's own line, in the pattern's frame, or
/// an angle in degrees for a span. Nothing here rounds to the document's step:
/// that belongs to the caller, which is where the modifier keys are read.
pub fn apply_grip(params: &mut Params, grip: &Grip, value: f64) {
    match grip.drive {
        Drive::Length { keys, base, per, min } => {
            let per = if per.abs() > 1e-9 { per } else { 1.0 };
            let v = ((value - base) / per).max(min);
            if keys.len() == 3 {
                // A run that can point anywhere: its direction is kept and only
                // its length is written.
                let step = Vec3::new(params.num(keys[0]), params.num(keys[1]), params.num(keys[2]));
                let length = step.length();
                let dir = if length > 1e-9 { step * (1.0 / length) } else { unit(0) };
                let scaled = dir * v;
                for (axis, key) in keys.iter().enumerate() {
                    let component = match axis {
                        0 => scaled.x,
                        1 => scaled.y,
                        _ => scaled.z,
                    };
                    params.insert((*key).to_string(), ParamValue::Length(component));
                }
            } else {
                params.insert(keys[0].to_string(), ParamValue::Length(v));
            }
        }
        Drive::Count { key, base, per } => {
            let per = if per.abs() > 1e-9 { per } else { 1.0 };
            // The same 1..512 the count parameters carry, so a grip can never
            // write a number the property editor would refuse.
            let count = ((value - base) / per).round().clamp(1.0, 512.0) as u32;
            params.insert(key.to_string(), ParamValue::Count(count));
        }
        Drive::Angle { key, per, .. } => {
            let per = if per.abs() > 1e-9 { per } else { 1.0 };
            params.insert(key.to_string(), ParamValue::Angle((value / per).clamp(-360.0, 360.0)));
        }
    }
}
