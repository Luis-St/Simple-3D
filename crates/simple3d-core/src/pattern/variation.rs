//! What changes a stage's copies from one to the next (issue 79).
//!
//! A stage places its copies -- a run, a ring -- and a *variation* then changes
//! each copy where it stands: moves it aside, turns it, resizes it, or opens the
//! gap before it. A stage holds a list of them, and each says what it changes,
//! along or about which axis, by how much, and which copies it reaches.
//!
//! Which copies is two numbers: *every* how many, *starting at* which. Every one
//! from the first is all of them, the original too; every other one from the
//! second is the brick bond. A variation that *builds up* gives the copies it
//! reaches one step more each time -- the first one reached gets one step, the
//! next two -- and one that *repeats* gives each of them the same single step.
//!
//! A variation used to be a shift vector, a spin, a size and a gap on a cycle
//! whose first copy was always the original, left alone, and four of them to a
//! stage. The cap was the slots rather than anything the rule could say, and
//! "every copy" and "the original as well" were the two answers the cycle could
//! not give. Now the list is as long as there are different things to say: a
//! stage takes one variation of each kind, axis and set of copies, and refuses
//! only a second one identical in all of those -- which would add nothing a
//! bigger amount on the first does not.

use super::*;
use crate::xform::Xform;
use simple3d_geom::Vec3;

/// What a variation changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Vary {
    /// Moves the copy aside along one axis, in the frame the stage placed it in.
    Shift,
    /// Turns the copy about one of its own axes.
    Spin,
    /// Makes the copy bigger or smaller along one axis, or all three.
    Size,
    /// Opens the gap before the copy, along the run. A run's only: a turn has
    /// no gaps, only an angle.
    Gap,
}

/// The kinds in the order a variation's choice holds them.
pub const VARIES: &[&str] = &["Shift", "Spin", "Size", "Gap"];

/// How a variation's amount steps from one copy it reaches to the next, in the
/// order its choice holds them.
pub const STEPS: &[&str] = &["Builds up", "Repeats"];

/// What a variation is along or about. The fourth is a size's only: all three
/// axes at once, which is what "each copy a tenth smaller" means.
pub const VARY_AXES: &[&str] = &["X", "Y", "Z", "All"];

/// The index of [`VARY_AXES`]'s "All".
pub const ALL_AXES: usize = 3;

impl Vary {
    pub const ALL: [Vary; 4] = [Vary::Shift, Vary::Spin, Vary::Size, Vary::Gap];

    pub fn index(self) -> u32 {
        self as u32
    }

    pub fn from_index(index: u32) -> Vary {
        Vary::ALL.get(index as usize).copied().unwrap_or(Vary::Shift)
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

    /// The axes it can be along or about, in the order they are offered. A gap
    /// is along the run, and has none to choose.
    pub fn axes(self) -> &'static [usize] {
        match self {
            Vary::Shift | Vary::Spin => &[0, 1, 2],
            Vary::Size => &[ALL_AXES, 0, 1, 2],
            Vary::Gap => &[0],
        }
    }
}

/// One thing a stage does to its copies, beyond placing them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Variation {
    pub what: Vary,
    /// 0, 1 or 2 -- or [`ALL_AXES`] for a size. Nothing for a gap.
    pub axis: usize,
    /// How much one step is: a distance for a shift and a gap, degrees for a
    /// spin, and a factor for a size -- 1 is no change, 0.9 a tenth smaller.
    pub amount: f64,
    /// Whether every copy it reaches gets the same single step, rather than one
    /// more than the copy it reached before.
    pub repeats: bool,
    /// It reaches every this many copies. One at the least: every copy.
    pub every: u32,
    /// The first copy it reaches, counted from one -- the original.
    pub start: u32,
}

impl Variation {
    /// A variation of `what` that changes nothing yet, reaching every copy after
    /// the original -- for the constructors to fill in.
    pub const fn blank(what: Vary) -> Variation {
        let amount = if matches!(what, Vary::Size) { 1.0 } else { 0.0 };
        let axis = if matches!(what, Vary::Size) { ALL_AXES } else { 2 };
        Variation { what, axis, amount, repeats: false, every: 1, start: 2 }
    }

    pub fn shift(axis: usize, distance: f64) -> Variation {
        Variation { axis: axis.min(2), amount: distance, ..Variation::blank(Vary::Shift) }
    }

    pub fn spin(axis: usize, degrees: f64) -> Variation {
        Variation { axis: axis.min(2), amount: degrees, ..Variation::blank(Vary::Spin) }
    }

    /// Every copy `factor` times the size of the one before it, along `axis` or
    /// all three.
    pub fn resize(axis: usize, factor: f64) -> Variation {
        Variation { axis: axis.min(ALL_AXES), amount: factor, ..Variation::blank(Vary::Size) }
    }

    pub fn widen(gap: f64) -> Variation {
        Variation { amount: gap, ..Variation::blank(Vary::Gap) }
    }

    /// The same variation, giving every copy it reaches one step rather than
    /// building up, and reaching every `every` copies from the second: every
    /// other copy moved on, for two.
    pub fn repeating(self, every: u32) -> Variation {
        Variation { repeats: true, every: every.clamp(1, MAX_CYCLE), ..self }
    }

    /// The same variation, reaching every `every` copies from copy `start`.
    pub fn reaching(self, every: u32, start: u32) -> Variation {
        Variation { every: every.clamp(1, MAX_CYCLE), start: start.clamp(1, MAX_CYCLE), ..self }
    }

    /// The first copy it reaches, counted from nought.
    fn first(&self) -> u32 {
        self.start.max(1) - 1
    }

    /// Whether it reaches copy `i`, counted from nought.
    pub fn reaches(&self, i: u32) -> bool {
        i >= self.first() && (i - self.first()).is_multiple_of(self.every.max(1))
    }

    /// How many steps of it copy `i` gets.
    pub fn times(&self, i: u32) -> f64 {
        if !self.reaches(i) {
            0.0
        } else if self.repeats {
            1.0
        } else {
            ((i - self.first()) / self.every.max(1) + 1) as f64
        }
    }

    /// How many steps the copies before copy `i` got between them, which is how
    /// far along its run a gap variation puts that copy.
    fn times_before(&self, i: u32) -> f64 {
        if i <= self.first() {
            return 0.0;
        }
        // The copies it reached before `i`, and the steps they got.
        let reached = ((i - self.first() - 1) / self.every.max(1) + 1) as f64;
        if self.repeats {
            reached
        } else {
            reached * (reached + 1.0) / 2.0
        }
    }

    /// Whether it changes anything on a stage doing `mode`.
    pub fn acts(&self, mode: StageMode) -> bool {
        self.what.fits(mode) && self.changes()
    }

    /// Whether its amount is anything but no change.
    fn changes(&self) -> bool {
        match self.what {
            Vary::Size => (self.amount - 1.0).abs() > 1e-9,
            _ => self.amount.abs() > 1e-9,
        }
    }

    /// What makes it a different variation from another of the same kind: the
    /// axis, how it steps, and the copies it reaches. Two that agree on all of
    /// it are one variation with the amounts added up, so a stage holds one.
    pub fn combination(&self) -> (Vary, usize, bool, u32, u32) {
        let axis = if self.what == Vary::Gap { 0 } else { self.axis };
        (self.what, axis, self.repeats, self.every.max(1), self.start.max(1))
    }

    /// What it does to copy `i` where it stands, or `None` where that is
    /// nothing -- a copy it gives no steps to, one it does not change, and any
    /// copy of a gap, which moves the copy along its run rather than about
    /// itself (see [`Variation::gap_before`]).
    pub(crate) fn reshape(&self, i: u32) -> Option<Xform> {
        let times = self.times(i);
        if times == 0.0 || !self.changes() {
            return None;
        }
        match self.what {
            Vary::Shift => Some(Xform::from_translation(unit(self.axis) * (self.amount * times))),
            Vary::Spin => Some(Xform::from_pos_rot(Vec3::ZERO, rotation_about(self.axis, self.amount * times))),
            Vary::Size => {
                // Kept above nothing: a hundred copies at 90 % each come out a
                // few hundred-thousandths of the first, and a copy of no size at
                // all is a degenerate matrix the rest of the program has no use
                // for.
                let size = self.amount.max(0.01).powf(times).max(1e-4);
                let scale = match self.axis {
                    ALL_AXES => Vec3::splat(size),
                    axis => Vec3::ONE + unit(axis) * (size - 1.0),
                };
                Some(Xform::from_pos_rot_scale(Vec3::ZERO, Vec3::ZERO, scale))
            }
            Vary::Gap => None,
        }
    }

    /// How much further along its run a gap variation puts copy `i`.
    pub(crate) fn gap_before(&self, i: u32) -> f64 {
        if self.what == Vary::Gap {
            self.amount * self.times_before(i)
        } else {
            0.0
        }
    }
}

/// The furthest apart the copies a variation reaches can be, and the latest
/// copy it can start at: the most copies a stage makes.
pub const MAX_CYCLE: u32 = 512;

/// The most variations one stage is written with. Not the limit a stage is
/// held to -- that is how many different variations its copies allow, see
/// [`free_variation`] -- but a bound on what a hand-edited file can ask for.
pub const MAX_VARIATIONS: u32 = 9999;

/// The variations one variation of the old list comes to (issue 79).
///
/// The old list reached every copy after the original and, where it repeated,
/// came round every `every` copies with a *count* of steps -- nought, one, two,
/// nought -- rather than one step on the copies it reached. `once` is the same
/// variation as a single step; what is returned lays every copy down where the
/// old one did. Building up is the same thing in both. Coming round every two
/// is one step on every other copy from the second. A longer cycle is one
/// variation for each count of steps it gave, reaching the copies it gave that
/// many to.
pub(crate) fn from_cycle(once: Variation, every: Option<u32>) -> Vec<Variation> {
    let Some(every) = every.map(|e| e.clamp(2, 64)) else {
        return vec![once.reaching(1, 2)];
    };
    (1..every)
        .map(|steps| {
            let amount = match once.what {
                Vary::Size => once.amount.max(0.01).powi(steps as i32),
                _ => once.amount * steps as f64,
            };
            Variation { amount, repeats: true, ..once }.reaching(every, steps + 1)
        })
        .collect()
}
