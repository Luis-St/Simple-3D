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
// kind can say. Every kind above is one or more stages -- which is not only the
// check that the model is the right one, but what the fixed kinds are now laid
// out *by* (see [`rule_stages`]).

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

/// One stage of a custom rule: how many copies it makes, what it does to
/// place each of them, and what varies them from one to the next.
///
/// The transform of copy `i` is worked out from `i` directly rather than by
/// composing the stage with itself `i` times. That is what makes a stage able to
/// say "the radius grows 5 mm a copy" -- repeated composition would carry the
/// growth round the turn with it and draw an involute instead of a spiral -- and
/// it is what lets the copies *vary* at all: a gap that widens, a shift that
/// comes round every other copy, a size that shrinks are all a function of `i`
/// and of nothing a previous copy did.
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
    /// How many of `vary` are in use.
    pub varied: usize,
    /// What changes the copies from one to the next, applied in this order
    /// (issue 79). Only the first `varied` of them mean anything; the rest are
    /// blank, so two stages that vary their copies alike compare equal.
    pub vary: [Variation; MAX_VARIATIONS],
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
            varied: 0,
            vary: [Variation::blank(Vary::Shift); MAX_VARIATIONS],
        }
    }

    /// The same stage with `variation` on the end of its list. A full list is
    /// left as it is.
    pub fn with(mut self, variation: Variation) -> Stage {
        if self.varied < MAX_VARIATIONS {
            self.vary[self.varied] = variation;
            self.varied += 1;
        }
        self
    }

    /// The variations in use, in the order they are applied.
    pub fn variations(&self) -> &[Variation] {
        &self.vary[..self.varied.min(MAX_VARIATIONS)]
    }

    /// Whether the stage changes its copies from one to the next rather than
    /// only placing them (issue 79).
    pub fn varies(&self) -> bool {
        self.variations().iter().any(|variation| variation.acts(self.mode))
    }

    /// The copies this stage makes, in the frame of whatever it is repeating.
    pub fn instances(&self) -> Vec<Instance> {
        if self.mode == StageMode::Mirror {
            let mut m = Xform::IDENTITY;
            m.m[self.axis][self.axis] = -1.0;
            return vec![Instance::plain(Xform::IDENTITY), Instance { xform: m, mirrored: true }];
        }
        (0..self.count.max(1)).map(|i| Instance::plain(self.place(i))).collect()
    }

    /// Where copy `i` goes: placed by the stage's run or turn, then changed by
    /// each of its variations in turn, in its own frame.
    ///
    /// The variations are composed on the *inside*, the way the scatter is: a
    /// shifted brick moves along the row it is in, not across whatever the row
    /// happens to be turned to, and a spinning copy turns about its own origin
    /// rather than swinging round the centre of the stage. In order, so a shift
    /// listed before a spin moves the copy and then turns it where it landed,
    /// and one listed after it moves the copy along the way it now faces. A
    /// stage that varies nothing composes nothing, so its copies are bit for bit
    /// what they were before a stage could vary them.
    pub fn place(&self, i: u32) -> Xform {
        let n = i as f64;
        let placed = match self.mode {
            StageMode::Turn => turned(self.axis, self.radius + self.growth * n, self.turn * n, self.rise * n),
            _ => Xform::from_translation(self.along(i)),
        };
        let mut own: Option<Xform> = None;
        for variation in self.variations().iter().filter(|v| v.what.fits(self.mode)) {
            if let Some(change) = variation.reshape(i) {
                own = Some(match own {
                    Some(before) => before.compose(&change),
                    None => change,
                });
            }
        }
        match own {
            Some(own) => placed.compose(&own),
            None => placed,
        }
    }

    /// How far along its run copy `i` sits: `i` steps, plus whatever the gap
    /// variations have added to the gaps before it.
    fn along(&self, i: u32) -> Vec3 {
        let run = self.step * i as f64;
        let length = self.step.length();
        if self.mode != StageMode::Move || length < 1e-9 {
            return run;
        }
        let extra: f64 = self.variations().iter().map(|variation| variation.gap_before(i)).sum();
        if extra.abs() < 1e-12 {
            return run;
        }
        run + self.step * (extra / length)
    }

    /// How much wider than the step the gap after copy `j` is.
    pub fn gap_after(&self, j: u32) -> f64 {
        if self.mode != StageMode::Move {
            return 0.0;
        }
        self.variations().iter().filter(|v| v.what == Vary::Gap).map(|v| v.gap * v.times(j)).sum()
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
    let index = index.min(MAX_STAGES - 1);
    let k = &STAGES[index];
    let varied = variation_count(params, index);
    let mut vary = [Variation::blank(Vary::Shift); MAX_VARIATIONS];
    for (slot, keys) in k.vary.iter().enumerate().take(varied) {
        vary[slot] = read_variation(params, keys);
    }
    Stage {
        mode: StageMode::from_index(params.int(k.mode)),
        count: params.int(k.count).max(1),
        step: Vec3::new(params.num(k.step[0]), params.num(k.step[1]), params.num(k.step[2])),
        turn: params.num(k.turn),
        axis: params.int(k.axis).min(2) as usize,
        radius: params.num(k.radius),
        growth: params.num(k.growth),
        rise: params.num(k.rise),
        varied,
        vary,
    }
}

/// Write one stage back into a pattern's parameters -- its variations too, and
/// the slots past the last one it uses as blank.
pub fn set_stage(params: &mut Params, index: usize, stage: Stage) {
    let k = &STAGES[index.min(MAX_STAGES - 1)];
    params.insert(k.mode.to_string(), ParamValue::Choice(stage.mode.index()));
    params.insert(k.count.to_string(), ParamValue::Count(stage.count.clamp(1, 512)));
    for (key, component) in k.step.iter().zip([stage.step.x, stage.step.y, stage.step.z]) {
        params.insert((*key).to_string(), ParamValue::Length(component));
    }
    params.insert(k.turn.to_string(), ParamValue::Angle(stage.turn.clamp(-360.0, 360.0)));
    params.insert(k.axis.to_string(), ParamValue::Choice(stage.axis.min(2) as u32));
    params.insert(k.radius.to_string(), ParamValue::Length(stage.radius));
    params.insert(k.growth.to_string(), ParamValue::Length(stage.growth));
    params.insert(k.rise.to_string(), ParamValue::Length(stage.rise));
    let varied = stage.varied.min(MAX_VARIATIONS);
    params.insert(k.varied.to_string(), ParamValue::Count(varied as u32));
    for (slot, keys) in k.vary.iter().enumerate() {
        let blank = Variation::blank(Vary::Shift);
        write_variation(params, keys, if slot < varied { &stage.vary[slot] } else { &blank });
    }
}
