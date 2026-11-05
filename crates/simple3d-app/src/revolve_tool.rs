//! Revolve (issue 74): the sixth tool on the rail, configured in an in-place popup. A closed region
//! of a kept sketch, or a flat face of the model, is turned about a line in its plane: a line of the
//! sketch or a straight edge of the model clicked in the viewport, or an axis of the world chosen in
//! the popup. The angle and its direction are typed or scrubbed there, the solid is shown live, and
//! Apply keeps it as a feature node that follows its sketch when it changes.
//!
//! Turned out of the profile's front it adds; turned into the object the profile lies on, it cuts.

mod angle;
mod axis;
mod commit;
pub(crate) mod draw;
mod interact;
mod window;
pub(crate) use draw::{draw, template};
pub(crate) use interact::{interact, step_hint};
pub(crate) use window::show;
#[cfg(test)]
mod tests;

use crate::push_pull_tool::Target;
use simple3d_core::scene::RevolveAxis;
use simple3d_geom::Vec3;
use std::sync::Arc;

/// The popup's key, which also remembers where it was dragged.
const KEY: &str = "revolve-tool";

/// The window width: a label and its field.
const WIDTH: f32 = 300.0;

/// The tool's state between frames: the profile, the axis and the turn the popup sets.
pub struct Revolve {
    pub(crate) profile: Option<Target>,
    pub(crate) axis: Option<AxisPick>,
    /// The turn in degrees, 1 to 360.
    pub(crate) angle: f64,
    /// Whether the profile turns left-handed about the axis rather than right-handed.
    pub(crate) flip: bool,
    /// The solid for the picks at a signed turn, or why there is none; `None` until remade.
    pub(crate) preview: Option<(f64, Result<Arc<crate::render::Renderable>, String>)>,
    /// Counts the remade previews, so the picture redraws whenever the solid changes.
    pub(crate) generation: u64,
}

impl Default for Revolve {
    fn default() -> Revolve {
        Revolve { profile: None, axis: None, angle: 360.0, flip: false, preview: None, generation: 0 }
    }
}

/// Where an axis came from, so the popup can show it and a new profile can take the same one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum AxisSource {
    /// A line of the sketch or an edge of the model, clicked in the viewport.
    Picked,
    /// The world's X, Y or Z axis, or the line parallel to it in the profile's plane.
    World(usize),
}

/// A line to turn about, in the world and in the profile's plane.
#[derive(Clone)]
pub(crate) struct AxisPick {
    pub(crate) world: (Vec3, Vec3),
    /// Its ends in the profile's plane.
    pub(crate) flat: ([f64; 2], [f64; 2]),
    pub(crate) stored: RevolveAxis,
    /// The middle of the profile in its plane, for which side of the axis it lies.
    pub(crate) middle: [f64; 2],
    pub(crate) source: AxisSource,
}

impl Revolve {
    /// The turn signed right-handed about the axis.
    pub(crate) fn signed_angle(&self) -> f64 {
        if self.flip {
            -self.angle
        } else {
            self.angle
        }
    }

    /// Drop the last pick: the axis, else the profile. Returns whether anything went.
    pub(crate) fn back(&mut self) -> bool {
        self.preview = None;
        if self.axis.take().is_some() {
            return true;
        }
        self.profile.take().is_some()
    }

    /// Drop the picks, keeping the angle and direction for the next profile.
    pub(crate) fn clear(&mut self) {
        let (angle, flip) = (self.angle, self.flip);
        *self = Revolve { angle, flip, generation: self.generation, ..Revolve::default() };
    }

    /// Take `profile`, keeping a chosen world axis if it still lies in the new profile's plane.
    pub(crate) fn set_profile(&mut self, profile: Target) {
        let world = match self.axis.as_ref().map(|axis| axis.source) {
            Some(AxisSource::World(k)) => Some(k),
            _ => None,
        };
        self.axis = world.and_then(|k| axis::world(&profile, k).ok());
        self.profile = Some(profile);
        self.preview = None;
    }

    pub(crate) fn set_axis(&mut self, axis: AxisPick) {
        self.axis = Some(axis);
        self.preview = None;
    }
}
