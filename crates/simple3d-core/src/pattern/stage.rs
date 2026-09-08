//! A stage of a custom pattern: reading one out and writing one back.

use super::*;
use crate::primitive::{ParamValue, Params, ParamsExt};
use crate::xform::Xform;
use simple3d_geom::Vec3;

// -- custom kinds (issue 67) -------------------------------------------------
//
// The six kinds above are the ones worth having a name for. A custom kind is
// the rule underneath all of them, spelled out: a stack of *stages*, each one
// repeating whatever the stages before it made. One stage stepping along X is a
// linear pattern; a second stepping along Y makes it a grid; a stage that turns
// about Z at a radius makes a ring, and a ring of rows is something no fixed
// kind can say. Every kind above can be written as one or two stages, which is
// the check that the model is the right one rather than a seventh special case.

/// One stage of a custom rule: how many copies it makes, and what it does to
/// each of them.
///
/// The transform of copy `i` is worked out from `i` directly rather than by
/// composing the stage with itself `i` times. That is what makes a stage able to
/// say "the radius grows 5 mm a copy" -- repeated composition would carry the
/// growth round the turn with it and draw an involute instead of a spiral -- and
/// it is the same arithmetic the fixed kinds do, so they come out identical.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stage {
    pub count: u32,
    /// Moved this far further along for each copy.
    pub step: Vec3,
    /// Turned this much further about `axis` for each copy.
    pub turn: f64,
    pub axis: usize,
    /// How far out from the axis the first copy sits.
    pub radius: f64,
    /// How much further out each copy after it sits.
    pub growth: f64,
    /// The stage is a reflection across the plane through the origin whose
    /// normal is `axis`: the original and its mirror image, and nothing else.
    pub mirror: bool,
}

impl Stage {
    /// The copies this stage makes, in the frame of whatever it is repeating.
    pub fn instances(&self) -> Vec<Instance> {
        if self.mirror {
            let mut m = Xform::IDENTITY;
            m.m[self.axis][self.axis] = -1.0;
            return vec![Instance::plain(Xform::IDENTITY), Instance { xform: m, mirrored: true }];
        }
        (0..self.count.max(1))
            .map(|i| {
                let i = i as f64;
                let placed = turned(self.axis, self.radius + self.growth * i, self.turn * i, 0.0);
                Instance::plain(Xform::from_translation(self.step * i).compose(&placed))
            })
            .collect()
    }

    /// How many copies it makes. A mirror is always two.
    pub fn copies(&self) -> usize {
        if self.mirror {
            2
        } else {
            self.count.max(1) as usize
        }
    }

    /// Whether the stage does anything at all: one copy that does not move is a
    /// stage the user has not filled in yet.
    pub fn is_idle(&self) -> bool {
        !self.mirror && self.count.max(1) == 1
    }
}

/// Read one stage out of a pattern's parameters.
pub fn stage(params: &Params, index: usize) -> Stage {
    let k = &STAGES[index.min(MAX_STAGES - 1)];
    Stage {
        count: params.int(k.count).max(1),
        step: Vec3::new(params.num(k.step[0]), params.num(k.step[1]), params.num(k.step[2])),
        turn: params.num(k.turn),
        axis: params.int(k.axis).min(2) as usize,
        radius: params.num(k.radius),
        growth: params.num(k.growth),
        mirror: params.flag(k.mirror),
    }
}

/// Write one stage back into a pattern's parameters.
pub fn set_stage(params: &mut Params, index: usize, stage: Stage) {
    let k = &STAGES[index.min(MAX_STAGES - 1)];
    params.insert(k.count.to_string(), ParamValue::Count(stage.count.clamp(1, 512)));
    for (axis, key) in k.step.iter().enumerate() {
        let component = match axis {
            0 => stage.step.x,
            1 => stage.step.y,
            _ => stage.step.z,
        };
        params.insert((*key).to_string(), ParamValue::Length(component));
    }
    params.insert(k.turn.to_string(), ParamValue::Angle(stage.turn.clamp(-360.0, 360.0)));
    params.insert(k.axis.to_string(), ParamValue::Choice(stage.axis.min(2) as u32));
    params.insert(k.radius.to_string(), ParamValue::Length(stage.radius));
    params.insert(k.growth.to_string(), ParamValue::Length(stage.growth));
    params.insert(k.mirror.to_string(), ParamValue::Bool(stage.mirror));
}
