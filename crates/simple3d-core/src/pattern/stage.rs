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

/// The three things a stage can be asked to do (issue 79).
///
/// A stage used to carry every number at once -- a run, a turn, a radius, a
/// growth and a mirror flag -- and show all nine of them whatever it was
/// actually doing, so "3 Radius per copy" sat under a stage that was a straight
/// run and meant nothing there. What a stage does is one choice out of three,
/// and saying so first is what lets the rest of the stage be the four or five
/// numbers that choice actually needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageMode {
    /// A run: each copy a fixed step further along.
    Move,
    /// A ring, a helix or a spiral: each copy turned further about an axis, at
    /// a radius that may grow and a height that may climb.
    Turn,
    /// The original and its reflection across a plane through the origin.
    Mirror,
}

/// The modes in the order they appear in a stage's "does" choice; the index
/// into this list is the value the choice parameter holds.
pub const STAGE_MODES: &[&str] = &["Move", "Turn", "Mirror"];

impl StageMode {
    pub fn index(self) -> u32 {
        match self {
            StageMode::Move => 0,
            StageMode::Turn => 1,
            StageMode::Mirror => 2,
        }
    }

    pub fn from_index(index: u32) -> StageMode {
        match index {
            1 => StageMode::Turn,
            2 => StageMode::Mirror,
            _ => StageMode::Move,
        }
    }
}

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
    pub mode: StageMode,
    pub count: u32,
    /// Moved this far further along for each copy. [`StageMode::Move`] only.
    pub step: Vec3,
    /// Turned this much further about `axis` for each copy.
    pub turn: f64,
    pub axis: usize,
    /// How far out from the axis the first copy sits.
    pub radius: f64,
    /// How much further out each copy after it sits.
    pub growth: f64,
    /// How far along `axis` each copy after the first climbs -- what turns a
    /// ring into a helix.
    pub rise: f64,
}

impl Stage {
    /// A run of `count` copies, each `step` further along.
    pub fn run(count: u32, step: Vec3) -> Stage {
        Stage { mode: StageMode::Move, count, step, ..Stage::still() }
    }

    /// A ring, helix or spiral about `axis`.
    pub fn turning(count: u32, turn: f64, radius: f64, growth: f64, rise: f64, axis: usize) -> Stage {
        Stage { mode: StageMode::Turn, count, turn, radius, growth, rise, axis, ..Stage::still() }
    }

    pub fn mirrored(axis: usize) -> Stage {
        Stage { mode: StageMode::Mirror, axis, ..Stage::still() }
    }

    /// A stage that does nothing, for the other constructors to fill in.
    fn still() -> Stage {
        Stage {
            mode: StageMode::Move,
            count: 1,
            step: Vec3::ZERO,
            turn: 0.0,
            axis: 2,
            radius: 0.0,
            growth: 0.0,
            rise: 0.0,
        }
    }

    /// The copies this stage makes, in the frame of whatever it is repeating.
    pub fn instances(&self) -> Vec<Instance> {
        if self.mode == StageMode::Mirror {
            let mut m = Xform::IDENTITY;
            m.m[self.axis][self.axis] = -1.0;
            return vec![Instance::plain(Xform::IDENTITY), Instance { xform: m, mirrored: true }];
        }
        (0..self.count.max(1))
            .map(|i| {
                let i = i as f64;
                Instance::plain(match self.mode {
                    StageMode::Turn => turned(self.axis, self.radius + self.growth * i, self.turn * i, self.rise * i),
                    _ => Xform::from_translation(self.step * i),
                })
            })
            .collect()
    }

    /// How many copies it makes. A mirror is always two.
    pub fn copies(&self) -> usize {
        if self.mode == StageMode::Mirror {
            2
        } else {
            self.count.max(1) as usize
        }
    }
}

/// Read one stage out of a pattern's parameters.
pub fn stage(params: &Params, index: usize) -> Stage {
    let k = &STAGES[index.min(MAX_STAGES - 1)];
    Stage {
        mode: StageMode::from_index(params.int(k.mode)),
        count: params.int(k.count).max(1),
        step: Vec3::new(params.num(k.step[0]), params.num(k.step[1]), params.num(k.step[2])),
        turn: params.num(k.turn),
        axis: params.int(k.axis).min(2) as usize,
        radius: params.num(k.radius),
        growth: params.num(k.growth),
        rise: params.num(k.rise),
    }
}

/// Write one stage back into a pattern's parameters.
pub fn set_stage(params: &mut Params, index: usize, stage: Stage) {
    let k = &STAGES[index.min(MAX_STAGES - 1)];
    params.insert(k.mode.to_string(), ParamValue::Choice(stage.mode.index()));
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
    params.insert(k.rise.to_string(), ParamValue::Length(stage.rise));
}
