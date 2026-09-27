//! The handles a pattern is dragged by, and what dragging one changes.

use super::*;
use crate::primitive::{ParamValue, Params, ParamsExt};
use simple3d_geom::Vec3;

/// One handle for laying a pattern out by eye.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grip {
    /// What it drives, for the tooltip and undo label. Unique per kind, so it identifies the grip
    /// across a drag where indices could shift.
    pub label: &'static str,
    /// Where the grip sits, in the pattern's own frame.
    pub at: Vec3,
    /// A point on the line it slides along, with a unit direction. For [`Drive::Angle`] the line is the
    /// turn axis, `from` its centre, and `radius` how far out the grip rides.
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

/// A length that may not go below zero: a radius or a run's length.
pub(crate) const POSITIVE: f64 = 0.0;

/// A length that may go either way: a step, rise or growth.
pub(crate) const EITHER_WAY: f64 = f64::NEG_INFINITY;

/// The grips a pattern offers, in its own frame; any at the origin (the move manipulator's spot,
/// and where a zero run puts everything) are dropped.
pub fn grips(params: &Params) -> Vec<Grip> {
    let mut out = match params.int("kind") {
        GRID => grid_grips(params),
        CIRCULAR => circular_grips(params),
        // A mirror has no distance or count to lay out.
        MIRROR => Vec::new(),
        HELIX => helix_grips(params),
        SPIRAL => spiral_grips(params),
        CUSTOM => custom_grips(params),
        _ => linear_grips(params),
    };
    out.retain(|g| g.at.length() > 1e-6);
    out
}

/// The grip with this label, for a drag that started on it.
pub fn grip(params: &Params, label: &str) -> Option<Grip> {
    grips(params).into_iter().find(|g| g.label == label)
}

/// Write what a grip was dragged to: a distance along its line in the pattern's frame, or a span
/// angle in degrees. Rounding to the step is the caller's job.
pub fn apply_grip(params: &mut Params, grip: &Grip, value: f64) {
    match grip.drive {
        Drive::Length { keys, base, per, min } => {
            let per = if per.abs() > 1e-9 { per } else { 1.0 };
            let v = ((value - base) / per).max(min);
            if keys.len() == 3 {
                // A run that can point anywhere: keep its direction, write its length.
                let step = Vec3::new(params.num(keys[0]), params.num(keys[1]), params.num(keys[2]));
                let length = step.length();
                let dir = if length > 1e-9 { step * (1.0 / length) } else { unit(0) };
                let scaled = dir * v;
                for (axis, key) in keys.iter().enumerate() {
                    let component = scaled.get(axis);
                    params.insert((*key).to_string(), ParamValue::Length(component));
                }
            } else {
                params.insert(keys[0].to_string(), ParamValue::Length(v));
            }
        }
        Drive::Count { key, base, per } => {
            let per = if per.abs() > 1e-9 { per } else { 1.0 };
            // The same 1..512 the count parameters allow.
            let count = ((value - base) / per).round().clamp(1.0, 512.0) as u32;
            params.insert(key.to_string(), ParamValue::Count(count));
        }
        Drive::Angle { key, per, .. } => {
            let per = if per.abs() > 1e-9 { per } else { 1.0 };
            params.insert(key.to_string(), ParamValue::Angle((value / per).clamp(-360.0, 360.0)));
        }
    }
}
