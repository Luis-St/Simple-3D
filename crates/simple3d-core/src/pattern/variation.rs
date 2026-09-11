//! What changes a stage's copies from one to the next (issue 79).
//!
//! A stage places its copies -- a run, a ring -- and a *variation* then changes
//! each copy where it stands: moves it aside, turns it, resizes it, or opens the
//! gap before it. A stage used to have exactly one of each, bundled under a
//! single "Vary" heading, and each stepped the one way it had been written to:
//! the shift came round on a cycle, the spin and the size built up copy by copy.
//! That could say "every other row moves on half a brick" and nothing more --
//! not a second shift on a cycle of three, and not a turn that flips every other
//! copy back again.
//!
//! A stage now holds a list of variations. Each says what it changes, by how
//! much, and how that amount steps from copy to copy -- building up (copy `i`
//! gets it `i` times) or coming round (copy `i` gets it `i mod n` times). The
//! same two answers for all four kinds, so a spin can alternate and a shift can
//! build up, and two variations of one kind on the same stage add up.

use super::*;
use crate::primitive::{ParamValue, Params, ParamsExt};
use crate::xform::Xform;
use simple3d_geom::Vec3;

/// What a variation changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vary {
    /// Moves the copy aside, in the frame the stage placed it in.
    Shift,
    /// Turns the copy about its own origin.
    Spin,
    /// Makes the copy bigger or smaller about its own origin.
    Size,
    /// Opens the gap before the copy, along the run. A run's only: a turn has
    /// no gaps, only an angle.
    Gap,
}

/// The kinds in the order a variation's choice holds them.
pub const VARIES: &[&str] = &["Shift", "Spin", "Size", "Gap"];

/// How a variation's amount steps from one copy to the next, in the order its
/// choice holds them.
pub const STEPS: &[&str] = &["Builds up", "Repeats"];

impl Vary {
    pub const ALL: [Vary; 4] = [Vary::Shift, Vary::Spin, Vary::Size, Vary::Gap];

    pub fn index(self) -> u32 {
        match self {
            Vary::Shift => 0,
            Vary::Spin => 1,
            Vary::Size => 2,
            Vary::Gap => 3,
        }
    }

    pub fn from_index(index: u32) -> Vary {
        match index {
            1 => Vary::Spin,
            2 => Vary::Size,
            3 => Vary::Gap,
            _ => Vary::Shift,
        }
    }

    pub fn name(self) -> &'static str {
        VARIES[self.index() as usize]
    }

    /// Whether a stage doing `mode` has any use for it. A mirror is two copies
    /// nothing varies, and a turn has no gaps.
    pub fn fits(self, mode: StageMode) -> bool {
        match mode {
            StageMode::Mirror => false,
            StageMode::Turn => self != Vary::Gap,
            StageMode::Move => true,
        }
    }
}

/// One thing a stage does to its copies, beyond placing them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Variation {
    pub what: Vary,
    /// Whether the amount comes round again every `every` copies, rather than
    /// building up copy by copy.
    pub repeats: bool,
    /// How many copies a repeating variation runs over before it starts again.
    /// Two at the least: a cycle of one changes nothing.
    pub every: u32,
    /// How far each step moves the copy aside. [`Vary::Shift`] only.
    pub offset: Vec3,
    /// How far each step turns it, in degrees. [`Vary::Spin`] only.
    pub angle: f64,
    /// What it is turned about.
    pub axis: usize,
    /// How big each step makes it: 1 is no change, 0.9 a tenth smaller.
    /// [`Vary::Size`] only.
    pub size: f64,
    /// How much wider each step makes the gap before it. [`Vary::Gap`] only.
    pub gap: f64,
}

impl Variation {
    /// A variation of `what` that changes nothing yet, for the constructors
    /// to fill in -- and what an unused slot holds.
    pub const fn blank(what: Vary) -> Variation {
        Variation { what, repeats: false, every: 2, offset: Vec3::ZERO, angle: 0.0, axis: 2, size: 1.0, gap: 0.0 }
    }

    pub fn shift(offset: Vec3) -> Variation {
        Variation { offset, ..Variation::blank(Vary::Shift) }
    }

    pub fn spin(angle: f64, axis: usize) -> Variation {
        Variation { angle, axis: axis.min(2), ..Variation::blank(Vary::Spin) }
    }

    pub fn resize(size: f64) -> Variation {
        Variation { size, ..Variation::blank(Vary::Size) }
    }

    pub fn widen(gap: f64) -> Variation {
        Variation { gap, ..Variation::blank(Vary::Gap) }
    }

    /// The same variation, coming round every `every` copies instead of
    /// building up.
    pub fn repeating(self, every: u32) -> Variation {
        Variation { repeats: true, every: every.clamp(2, 64), ..self }
    }

    /// How many steps of it copy `i` gets.
    pub fn times(&self, i: u32) -> f64 {
        if self.repeats {
            (i % self.every.max(2)) as f64
        } else {
            i as f64
        }
    }

    /// How many steps the gaps before copy `i` got between them, which is how
    /// far along its run a gap variation puts that copy.
    fn times_before(&self, i: u32) -> f64 {
        // The steps of gaps 0 to n - 1 of a variation that builds up.
        let triangle = |n: u64| (n * n.saturating_sub(1) / 2) as f64;
        if !self.repeats {
            return triangle(i as u64);
        }
        let every = self.every.max(2) as u64;
        (i as u64 / every) as f64 * triangle(every) + triangle(i as u64 % every)
    }

    /// Whether it changes anything on a stage doing `mode`.
    pub fn acts(&self, mode: StageMode) -> bool {
        self.what.fits(mode)
            && match self.what {
                Vary::Shift => self.offset.length() > 1e-9,
                Vary::Spin => self.angle.abs() > 1e-9,
                Vary::Size => (self.size - 1.0).abs() > 1e-9,
                Vary::Gap => self.gap.abs() > 1e-9,
            }
    }

    /// What it does to copy `i` where it stands, or `None` where that is
    /// nothing -- a copy it gives no steps to, one it does not change, and any
    /// copy of a gap, which moves the copy along its run rather than about
    /// itself (see [`Variation::gap_before`]).
    pub(crate) fn reshape(&self, i: u32) -> Option<Xform> {
        let times = self.times(i);
        if times == 0.0 {
            return None;
        }
        match self.what {
            Vary::Shift if self.offset.length() > 1e-9 => Some(Xform::from_translation(self.offset * times)),
            Vary::Spin if self.angle.abs() > 1e-9 => {
                Some(Xform::from_pos_rot(Vec3::ZERO, rotation_about(self.axis, self.angle * times)))
            }
            Vary::Size if (self.size - 1.0).abs() > 1e-9 => {
                // Kept above nothing: a hundred copies at 90 % each come out a
                // few hundred-thousandths of the first, and a copy of no size at
                // all is a degenerate matrix the rest of the program has no use
                // for.
                let size = self.size.max(0.01).powf(times).max(1e-4);
                Some(Xform::from_pos_rot_scale(Vec3::ZERO, Vec3::ZERO, Vec3::splat(size)))
            }
            _ => None,
        }
    }

    /// How much further along its run a gap variation puts copy `i`.
    pub(crate) fn gap_before(&self, i: u32) -> f64 {
        if self.what == Vary::Gap {
            self.gap * self.times_before(i)
        } else {
            0.0
        }
    }
}

/// Read one variation out of a pattern's parameters.
pub(crate) fn read_variation(params: &Params, k: &VaryKeys) -> Variation {
    // Read with their defaults in hand, as a stage's own are: a zero size is a
    // copy shrunk to nothing, not "no change".
    let whole = |key: &str, default: u32| params.get(key).map_or(default, |v| v.as_u32());
    Variation {
        what: Vary::from_index(params.int(k.what)),
        repeats: params.int(k.steps) == 1,
        every: whole(k.every, 2).clamp(2, 64),
        offset: Vec3::new(params.num(k.offset[0]), params.num(k.offset[1]), params.num(k.offset[2])),
        angle: params.num(k.angle),
        axis: whole(k.axis, 2).min(2) as usize,
        size: whole(k.size, 100).clamp(10, 1000) as f64 / 100.0,
        gap: params.num(k.gap),
    }
}

/// Write one variation back into a pattern's parameters.
pub(crate) fn write_variation(params: &mut Params, k: &VaryKeys, v: &Variation) {
    params.insert(k.what.to_string(), ParamValue::Choice(v.what.index()));
    params.insert(k.steps.to_string(), ParamValue::Choice(u32::from(v.repeats)));
    params.insert(k.every.to_string(), ParamValue::Count(v.every.clamp(2, 64)));
    for (key, component) in k.offset.iter().zip([v.offset.x, v.offset.y, v.offset.z]) {
        params.insert((*key).to_string(), ParamValue::Length(component));
    }
    params.insert(k.angle.to_string(), ParamValue::Angle(v.angle.clamp(-360.0, 360.0)));
    params.insert(k.axis.to_string(), ParamValue::Choice(v.axis.min(2) as u32));
    let percent = (v.size * 100.0).round().clamp(10.0, 1000.0) as u32;
    params.insert(k.size.to_string(), ParamValue::Count(percent));
    params.insert(k.gap.to_string(), ParamValue::Length(v.gap));
}
