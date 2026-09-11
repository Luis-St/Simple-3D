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

/// One stage of a custom rule: how many copies it makes, and what it does to
/// each of them.
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
    /// How much wider each gap of a run is than the one before it, so the
    /// copies spread out (or crowd in) along it. [`StageMode::Move`] only.
    pub gap_growth: f64,
    /// Added to a copy once for each place it sits into its cycle: copy `i` is
    /// moved by `shift * (i mod shift_every)`. A cycle of two shifts every
    /// other copy -- a row of bricks offset by half a brick, a hexagon grid.
    pub shift: Vec3,
    pub shift_every: u32,
    /// How far each copy turns about `axis`, where it stands, per copy.
    pub spin: f64,
    /// How big each copy is next to the one before it: 1 is the same size,
    /// 0.9 a tenth smaller every copy.
    pub scale: f64,
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
            gap_growth: 0.0,
            shift: Vec3::ZERO,
            shift_every: 2,
            spin: 0.0,
            scale: 1.0,
        }
    }

    /// Whether the stage changes its copies from one to the next rather than
    /// only placing them (issue 79). What opens the tool's "Vary" section by
    /// itself: a stage using any of it has to show it.
    pub fn varies(&self) -> bool {
        self.mode != StageMode::Mirror
            && (self.reshapes() || (self.mode == StageMode::Move && self.gap_growth.abs() > 1e-9))
    }

    /// Whether any copy is shifted, spun or resized where it stands.
    fn reshapes(&self) -> bool {
        self.shift.length() > 1e-9 || self.spin.abs() > 1e-9 || (self.scale - 1.0).abs() > 1e-9
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

    /// Where copy `i` goes: placed by the stage's run or turn, then shifted,
    /// spun and resized in its own frame.
    ///
    /// The variation is composed on the *inside*, the way the scatter is: a
    /// shifted brick moves along the row it is in, not across whatever the row
    /// happens to be turned to, and a spinning copy turns about its own origin
    /// rather than swinging round the centre of the stage. A stage that varies
    /// nothing composes nothing, so its copies are bit for bit what they were
    /// before a stage could vary them.
    pub fn place(&self, i: u32) -> Xform {
        let n = i as f64;
        let placed = match self.mode {
            StageMode::Turn => turned(self.axis, self.radius + self.growth * n, self.turn * n, self.rise * n),
            _ => Xform::from_translation(self.along(n)),
        };
        if !self.reshapes() {
            return placed;
        }
        let offset = self.shift * (i % self.shift_every.max(2)) as f64;
        // Kept above nothing: a hundred copies at 90 % each come out a few
        // hundred-thousandths of the first, and a copy of no size at all is a
        // degenerate matrix the rest of the program has no use for.
        let size = self.scale.max(0.01).powf(n).max(1e-4);
        placed.compose(&Xform::from_pos_rot_scale(offset, rotation_about(self.axis, self.spin * n), Vec3::splat(size)))
    }

    /// How far along its run copy `n` sits. Each gap is the step, plus the
    /// growth once for every gap before it, so the copy after `n` gaps is `n`
    /// steps and `n (n - 1) / 2` growths out.
    fn along(&self, n: f64) -> Vec3 {
        let run = self.step * n;
        let length = self.step.length();
        if self.gap_growth.abs() < 1e-12 || length < 1e-9 {
            return run;
        }
        run + self.step * (self.gap_growth * n * (n - 1.0) / 2.0 / length)
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
    // The two whose zero means something other than "no change" are read with
    // their default in hand: a map that lacks them -- a rule from before they
    // existed that has not been through the migration -- must not come out
    // shrinking every copy to a tenth.
    let whole = |key: &str, default: u32| params.get(key).map_or(default, |v| v.as_u32());
    Stage {
        mode: StageMode::from_index(params.int(k.mode)),
        count: params.int(k.count).max(1),
        step: Vec3::new(params.num(k.step[0]), params.num(k.step[1]), params.num(k.step[2])),
        turn: params.num(k.turn),
        axis: params.int(k.axis).min(2) as usize,
        radius: params.num(k.radius),
        growth: params.num(k.growth),
        rise: params.num(k.rise),
        gap_growth: params.num(k.gap_growth),
        shift: Vec3::new(params.num(k.shift[0]), params.num(k.shift[1]), params.num(k.shift[2])),
        shift_every: whole(k.shift_every, 2).clamp(2, 64),
        spin: params.num(k.spin),
        scale: whole(k.scale, 100).clamp(10, 1000) as f64 / 100.0,
    }
}

/// Write one stage back into a pattern's parameters.
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
    params.insert(k.gap_growth.to_string(), ParamValue::Length(stage.gap_growth));
    for (key, component) in k.shift.iter().zip([stage.shift.x, stage.shift.y, stage.shift.z]) {
        params.insert((*key).to_string(), ParamValue::Length(component));
    }
    params.insert(k.shift_every.to_string(), ParamValue::Count(stage.shift_every.clamp(2, 64)));
    params.insert(k.spin.to_string(), ParamValue::Angle(stage.spin.clamp(-360.0, 360.0)));
    let percent = (stage.scale * 100.0).round().clamp(10.0, 1000.0) as u32;
    params.insert(k.scale.to_string(), ParamValue::Count(percent));
}
