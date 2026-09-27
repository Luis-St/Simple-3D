//! A stage of a custom pattern: reading one out and writing one back.

use super::*;
use crate::primitive::{ParamValue, Params, ParamsExt};
use crate::xform::Xform;
use simple3d_geom::Vec3;

// -- custom kinds (issue 67) -------------------------------------------------
//
// A custom kind is a stack of stages, each repeating what the previous stages made. Every fixed
// kind is one or more stages and is laid out by them (see [`rule_stages`]).

/// The three things a stage can do (issue 79). Choosing one first lets the stage show only the
/// numbers that choice needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageMode {
    /// A run: each copy a fixed step further along.
    Move,
    /// A ring, helix or spiral: each copy turned further about an axis, at a radius that may grow and
    /// a height that may climb.
    Turn,
    /// The original and its reflection across a plane through the origin.
    Mirror,
}

/// The modes in choice order; the index is the parameter's value.
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

/// One stage of a custom rule: its copy count, placement, and variations.
///
/// Copy `i` is computed from `i` directly rather than by composing the stage `i` times, so a
/// growing radius gives a spiral rather than an involute and variations can depend on `i` alone.
#[derive(Clone, Debug, PartialEq)]
pub struct Stage {
    pub mode: StageMode,
    pub count: u32,
    /// Moved this far further along per copy; [`StageMode::Move`] only.
    pub step: Vec3,
    /// Turned this much further about `axis` per copy.
    pub turn: f64,
    pub axis: usize,
    /// The first copy's distance from the axis.
    pub radius: f64,
    /// How much further out each later copy sits.
    pub growth: f64,
    /// How far along `axis` each later copy climbs: a ring becomes a helix.
    pub rise: f64,
    /// What changes the copies from one to the next, applied in order (issue 79).
    pub vary: Vec<Variation>,
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

    /// A no-op stage, for the other constructors to fill in.
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
            vary: Vec::new(),
        }
    }

    /// The same stage with `variation` appended.
    pub fn with(mut self, variation: Variation) -> Stage {
        self.vary.push(variation);
        self
    }

    /// The variations, in application order.
    pub fn variations(&self) -> &[Variation] {
        &self.vary
    }

    /// Whether the stage varies its copies rather than only placing them (issue 79).
    pub fn varies(&self) -> bool {
        self.variations().iter().any(|variation| variation.acts(self.mode))
    }

    /// The copies this stage makes, in the frame of whatever it repeats.
    pub fn instances(&self) -> Vec<Instance> {
        if self.mode == StageMode::Mirror {
            let mut m = Xform::IDENTITY;
            m.m[self.axis][self.axis] = -1.0;
            return vec![Instance::plain(Xform::IDENTITY), Instance { xform: m, mirrored: true }];
        }
        (0..self.count.max(1)).map(|i| Instance::plain(self.place(i))).collect()
    }

    /// Where copy `i` goes: placed by the run or turn, then changed by each variation in order.
    ///
    /// Variations compose on the inside, like the scatter: a shift moves along the copy's own row and a
    /// spin turns about its own origin. A stage without variations is bit for bit what it was before.
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

    /// Copy `i`'s position along its run: `i` steps plus the gap variations before it.
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
        self.variations().iter().filter(|v| v.what == Vary::Gap).map(|v| v.amount * v.times(j)).sum()
    }

    /// How many copies it makes; a mirror is always two.
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
    let vary = (0..variation_count(params, index)).map(|slot| read_variation(params, index, slot)).collect();
    Stage {
        mode: StageMode::from_index(params.int(k.mode)),
        count: params.int(k.count).max(1),
        step: Vec3::new(params.num(k.step[0]), params.num(k.step[1]), params.num(k.step[2])),
        turn: params.num(k.turn),
        axis: params.int(k.axis).min(2) as usize,
        radius: params.num(k.radius),
        growth: params.num(k.growth),
        rise: params.num(k.rise),
        vary,
    }
}

/// Write one stage back, including its variations, clearing any leftovers of a longer list.
pub fn set_stage(params: &mut Params, index: usize, stage: &Stage) {
    let index = index.min(MAX_STAGES - 1);
    let k = &STAGES[index];
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
    let used = stage.vary.len().min(MAX_VARIATIONS as usize);
    params.insert(k.variations.to_string(), ParamValue::Count(used as u32));
    clear_variations_from(params, index, used);
    for (slot, variation) in stage.vary.iter().take(used).enumerate() {
        write_variation(params, index, slot, variation);
    }
}
