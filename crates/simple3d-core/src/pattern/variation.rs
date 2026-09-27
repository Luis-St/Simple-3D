//! What changes a stage's copies from one to the next (issue 79).
//!
//! A stage places its copies; each variation in its list then shifts, spins, resizes or opens
//! the gap before the copies it reaches, chosen by *every* and *starting at*. Building up gives
//! each reached copy one more step than the last; repeating gives each the same step.
//!
//! This replaced four fixed slots on a cycle that always skipped the original. A stage now takes
//! one variation per kind, axis, stepping and reach, refusing only exact duplicates.

use super::*;
use crate::xform::Xform;
use simple3d_geom::Vec3;

/// What a variation changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Vary {
    /// Moves the copy along one axis of the stage's frame.
    Shift,
    /// Turns the copy about one of its own axes.
    Spin,
    /// Scales the copy along one axis, or all three.
    Size,
    /// Opens the gap before the copy along the run; runs only.
    Gap,
}

/// The kinds in the order a variation's choice holds them.
pub const VARIES: &[&str] = &["Shift", "Spin", "Size", "Gap"];

/// How a variation's amount steps between reached copies, in choice order.
pub const STEPS: &[&str] = &["Builds up", "Repeats"];

/// What a variation is along or about; "All" is for size only.
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

    /// Whether it applies to `mode`: mirrors have nothing to vary, turns have no gaps.
    pub fn fits(self, mode: StageMode) -> bool {
        match mode {
            StageMode::Mirror => false,
            StageMode::Turn => self != Vary::Gap,
            StageMode::Move => true,
        }
    }

    /// The axes it can use, in offer order; a gap has none.
    pub fn axes(self) -> &'static [usize] {
        match self {
            Vary::Shift | Vary::Spin => &[0, 1, 2],
            Vary::Size => &[ALL_AXES, 0, 1, 2],
            Vary::Gap => &[0],
        }
    }
}

/// One thing a stage does to its copies beyond placing them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Variation {
    pub what: Vary,
    /// 0, 1 or 2, or [`ALL_AXES`] for a size; unused for a gap.
    pub axis: usize,
    /// One step: a distance for shift and gap, degrees for spin, a factor for size (1 is no change).
    pub amount: f64,
    /// Whether every reached copy gets the same single step rather than building up.
    pub repeats: bool,
    /// It reaches every this many copies; at least one.
    pub every: u32,
    /// The first copy it reaches, counted from one (the original).
    pub start: u32,
}

impl Variation {
    /// A no-op variation of `what`, reaching every copy after the original.
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

    /// Every copy `factor` times the previous one's size, along `axis` or all three.
    pub fn resize(axis: usize, factor: f64) -> Variation {
        Variation { axis: axis.min(ALL_AXES), amount: factor, ..Variation::blank(Vary::Size) }
    }

    pub fn widen(gap: f64) -> Variation {
        Variation { amount: gap, ..Variation::blank(Vary::Gap) }
    }

    /// Repeating one step on every `every` copies from the second.
    pub fn repeating(self, every: u32) -> Variation {
        Variation { repeats: true, every: every.clamp(1, MAX_CYCLE), ..self }
    }

    /// Reaching every `every` copies from copy `start`.
    pub fn reaching(self, every: u32, start: u32) -> Variation {
        Variation { every: every.clamp(1, MAX_CYCLE), start: start.clamp(1, MAX_CYCLE), ..self }
    }

    /// The first copy it reaches, counted from zero.
    fn first(&self) -> u32 {
        self.start.max(1) - 1
    }

    /// Whether it reaches copy `i`, counted from zero.
    pub fn reaches(&self, i: u32) -> bool {
        i >= self.first() && (i - self.first()).is_multiple_of(self.every.max(1))
    }

    /// How many steps copy `i` gets.
    pub fn times(&self, i: u32) -> f64 {
        if !self.reaches(i) {
            0.0
        } else if self.repeats {
            1.0
        } else {
            ((i - self.first()) / self.every.max(1) + 1) as f64
        }
    }

    /// Total steps of the copies before `i`: how far along the run a gap puts copy `i`.
    fn times_before(&self, i: u32) -> f64 {
        if i <= self.first() {
            return 0.0;
        }
        // The copies reached before `i`, and the steps they got.
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

    /// What distinguishes it from another of its kind: axis, stepping and reach. Two agreeing on all
    /// are one variation, so a stage holds only one.
    pub fn combination(&self) -> (Vary, usize, bool, u32, u32) {
        let axis = if self.what == Vary::Gap { 0 } else { self.axis };
        (self.what, axis, self.repeats, self.every.max(1), self.start.max(1))
    }

    /// Its in-place transform for copy `i`, or `None` if nothing (gaps move along the run instead,
    /// see [`Variation::gap_before`]).
    pub(crate) fn reshape(&self, i: u32) -> Option<Xform> {
        let times = self.times(i);
        if times == 0.0 || !self.changes() {
            return None;
        }
        match self.what {
            Vary::Shift => Some(Xform::from_translation(unit(self.axis) * (self.amount * times))),
            Vary::Spin => Some(Xform::from_pos_rot(Vec3::ZERO, rotation_about(self.axis, self.amount * times))),
            Vary::Size => {
                // Clamped above zero: a hundred copies at 90% shrink to almost nothing, and a zero scale is
                // a degenerate matrix.
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

/// The widest reach spacing and latest start: the most copies a stage makes.
pub const MAX_CYCLE: u32 = 512;

/// A bound on variations per stage for hand-edited files; the real limit is [`free_variation`].
pub const MAX_VARIATIONS: u32 = 9999;

/// The variations one old-style variation becomes (issue 79).
///
/// The old list reached every copy after the original and cycled step counts (0, 1, 2, 0).
/// `once` is the single-step variation; the result places every copy as before. A cycle of two is
/// one step on every other copy from the second; longer cycles become one variation per step count.
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
