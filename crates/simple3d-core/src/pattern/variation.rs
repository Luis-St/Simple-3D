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
use crate::primitive::{ParamKind, ParamSpec, ParamValue, Params};
use crate::xform::Xform;
use simple3d_geom::Vec3;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

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

/// The numbers one variation is stored under.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VaryField {
    What,
    Steps,
    Every,
    Start,
    Axis,
    Shift,
    Angle,
    Size,
    Gap,
}

impl VaryField {
    pub const ALL: [VaryField; 9] = [
        VaryField::What,
        VaryField::Steps,
        VaryField::Every,
        VaryField::Start,
        VaryField::Axis,
        VaryField::Shift,
        VaryField::Angle,
        VaryField::Size,
        VaryField::Gap,
    ];

    fn suffix(self) -> &'static str {
        match self {
            VaryField::What => "what",
            VaryField::Steps => "steps",
            VaryField::Every => "every",
            VaryField::Start => "start",
            VaryField::Axis => "axis",
            VaryField::Shift => "shift",
            VaryField::Angle => "angle",
            VaryField::Size => "size",
            VaryField::Gap => "gap",
        }
    }

    /// The field that holds the amount of a variation of `what`.
    pub fn amount_of(what: Vary) -> VaryField {
        match what {
            Vary::Shift => VaryField::Shift,
            Vary::Spin => VaryField::Angle,
            Vary::Size => VaryField::Size,
            Vary::Gap => VaryField::Gap,
        }
    }
}

/// The parameter one variation's number is stored under: stage `stage` and slot
/// `slot`, both from nought.
///
/// Built rather than tabled. A stage used to have four variation slots spelled
/// out as ordinary parameters, and four was therefore as many as it could have.
/// The names are the same shape for every slot, so they are made when asked for.
pub fn vary_key(stage: usize, slot: usize, field: VaryField) -> String {
    format!("stage{}_var{}_{}", stage.min(MAX_STAGES - 1) + 1, slot + 1, field.suffix())
}

/// Which stage, slot and field a variation's parameter is, from nought.
pub fn parse_vary_key(key: &str) -> Option<(usize, usize, VaryField)> {
    let rest = key.strip_prefix("stage")?;
    let (stage, rest) = rest.split_once("_var")?;
    let (slot, suffix) = rest.split_once('_')?;
    let stage: usize = stage.parse().ok()?;
    let slot: usize = slot.parse().ok()?;
    let field = VaryField::ALL.into_iter().find(|f| f.suffix() == suffix)?;
    (1..=MAX_STAGES).contains(&stage).then_some(())?;
    (slot >= 1).then_some((stage - 1, slot - 1, field))
}

/// What one variation's parameter is -- its kind, bounds, default and the label
/// its field answers to -- for any stage and slot.
///
/// The property editor, the tool and the migration all want a [`ParamSpec`],
/// and a spec's key and label are `'static`. They are made the first time a
/// slot is asked for and kept for the life of the program: a stage with a
/// dozen variations costs a dozen small entries, once.
pub fn vary_spec(stage: usize, slot: usize, field: VaryField) -> &'static ParamSpec {
    type Specs = HashMap<(usize, usize, VaryField), &'static ParamSpec>;
    static SPECS: OnceLock<Mutex<Specs>> = OnceLock::new();
    let stage = stage.min(MAX_STAGES - 1);
    let mut specs = SPECS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    specs.entry((stage, slot, field)).or_insert_with(|| {
        let key: &'static str = Box::leak(vary_key(stage, slot, field).into_boxed_str());
        let number = |name: &str| -> &'static str { Box::leak(format!("{}.{} {name}", stage + 1, slot + 1).into()) };
        let when = Some(("kind", CUSTOM));
        let (label, kind, default) = match field {
            VaryField::What => ("Varies", ParamKind::Choice { options: VARIES }, ParamValue::Choice(0)),
            VaryField::Steps => ("Steps", ParamKind::Choice { options: STEPS }, ParamValue::Choice(0)),
            VaryField::Axis => ("Axis", ParamKind::Choice { options: VARY_AXES }, ParamValue::Choice(2)),
            VaryField::Every => (number("Every"), ParamKind::Count { min: 1, max: MAX_CYCLE }, ParamValue::Count(1)),
            VaryField::Start => (number("Start at"), ParamKind::Count { min: 1, max: MAX_CYCLE }, ParamValue::Count(2)),
            VaryField::Shift => {
                (number("Shift"), ParamKind::Length { min: f64::NEG_INFINITY }, ParamValue::Length(0.0))
            }
            VaryField::Angle => {
                (number("Spin"), ParamKind::Angle { min: -360.0, max: 360.0, wrap: false }, ParamValue::Angle(0.0))
            }
            VaryField::Size => (number("Size (%)"), ParamKind::Count { min: 10, max: 1000 }, ParamValue::Count(100)),
            VaryField::Gap => (number("Gap"), ParamKind::Length { min: f64::NEG_INFINITY }, ParamValue::Length(0.0)),
        };
        Box::leak(Box::new(ParamSpec { key, label, kind, default, lock_group: 0, shown_when: when }))
    })
}

/// Any pattern parameter's spec: one from the table, or one of a variation's.
pub fn param_spec(key: &str) -> Option<&'static ParamSpec> {
    PARAMS
        .iter()
        .find(|spec| spec.key == key)
        .or_else(|| parse_vary_key(key).map(|(stage, slot, field)| vary_spec(stage, slot, field)))
}

/// Read variation `slot` of stage `stage` out of a pattern's parameters.
pub(crate) fn read_variation(params: &Params, stage: usize, slot: usize) -> Variation {
    // Read with their defaults in hand, as a stage's own are: a zero size is a
    // copy shrunk to nothing, not "no change".
    let value = |field: VaryField| {
        let spec = vary_spec(stage, slot, field);
        params.get(spec.key).copied().unwrap_or(spec.default)
    };
    let what = Vary::from_index(value(VaryField::What).as_u32());
    let amount = match what {
        Vary::Size => value(VaryField::Size).as_u32().clamp(10, 1000) as f64 / 100.0,
        _ => value(VaryField::amount_of(what)).as_f64(),
    };
    let most_axis = if what == Vary::Size { ALL_AXES } else { 2 };
    Variation {
        what,
        axis: (value(VaryField::Axis).as_u32() as usize).min(most_axis),
        amount,
        repeats: value(VaryField::Steps).as_u32() == 1,
        every: value(VaryField::Every).as_u32().clamp(1, MAX_CYCLE),
        start: value(VaryField::Start).as_u32().clamp(1, MAX_CYCLE),
    }
}

/// Write variation `slot` of stage `stage` back into a pattern's parameters.
/// Only the amount its own kind reads is written.
pub(crate) fn write_variation(params: &mut Params, stage: usize, slot: usize, v: &Variation) {
    let mut put = |field: VaryField, value: ParamValue| {
        params.insert(vary_key(stage, slot, field), value);
    };
    put(VaryField::What, ParamValue::Choice(v.what.index()));
    put(VaryField::Steps, ParamValue::Choice(u32::from(v.repeats)));
    put(VaryField::Every, ParamValue::Count(v.every.clamp(1, MAX_CYCLE)));
    put(VaryField::Start, ParamValue::Count(v.start.clamp(1, MAX_CYCLE)));
    put(VaryField::Axis, ParamValue::Choice(v.axis.min(ALL_AXES) as u32));
    match v.what {
        Vary::Shift => put(VaryField::Shift, ParamValue::Length(v.amount)),
        Vary::Spin => put(VaryField::Angle, ParamValue::Angle(v.amount.clamp(-360.0, 360.0))),
        Vary::Size => put(VaryField::Size, ParamValue::Count((v.amount * 100.0).round().clamp(10.0, 1000.0) as u32)),
        Vary::Gap => put(VaryField::Gap, ParamValue::Length(v.amount)),
    }
}

/// Take every parameter of stage `stage`'s variations from slot `from` on out
/// of the map, so a list that got shorter leaves nothing behind it.
pub(crate) fn clear_variations_from(params: &mut Params, stage: usize, from: usize) {
    params.retain(|key, _| !matches!(parse_vary_key(key), Some((s, slot, _)) if s == stage && slot >= from));
}

/// Every variation parameter of stage `stage`'s first `used` slots, in slot
/// order.
pub fn variation_keys(stage: usize, used: usize) -> Vec<String> {
    (0..used).flat_map(|slot| VaryField::ALL.map(|field| vary_key(stage, slot, field))).collect()
}

/// How many different variations of `what` a stage making `copies` copies can
/// hold: one for each axis, way of stepping, and set of copies it can reach.
pub fn combinations(what: Vary, copies: u32) -> usize {
    let copies = copies.clamp(1, MAX_CYCLE) as usize;
    what.axes().len() * STEPS.len() * copies * copies
}

/// Whether the stage has room for another variation of `what`: one that is not
/// the same as any it already holds.
pub fn has_room_for(stage: &Stage, what: Vary) -> bool {
    let copies = stage.copies() as u32;
    let taken: HashSet<_> = stage
        .variations()
        .iter()
        .filter(|v| v.what == what && v.every <= copies && v.start <= copies)
        .map(Variation::combination)
        .collect();
    what.fits(stage.mode) && taken.len() < combinations(what, copies)
}

/// `wanted`, or where the stage already holds a variation just like it, the
/// nearest one that differs: another axis first, then other copies to reach.
/// `None` where every combination is taken.
///
/// Searched rather than counted out, and only when a chip is pressed: a stage
/// of 512 copies has half a million ways to reach them, and the first free one
/// is nearly always the first or second tried.
pub fn free_variation(stage: &Stage, wanted: Variation) -> Option<Variation> {
    let copies = (stage.copies() as u32).clamp(1, MAX_CYCLE);
    // Reaching copies the stage has: every other copy of a stage of one is a
    // variation that changes nothing, and one the limit cannot count.
    let wanted = Variation { every: wanted.every.min(copies), start: wanted.start.min(copies), ..wanted };
    let taken = |v: &Variation| stage.variations().iter().any(|held| held.combination() == v.combination());
    let axes: Vec<usize> =
        std::iter::once(wanted.axis).chain(wanted.what.axes().iter().copied().filter(|a| *a != wanted.axis)).collect();
    // The same copies along another axis, which is nearly always the one that
    // was meant: a stagger across a second axis, a spin about a second one.
    if let Some(other) = axes.iter().map(|&axis| Variation { axis, ..wanted }).find(|v| !taken(v)) {
        return Some(other);
    }
    // Then other copies to reach, from the ones asked for outwards: the same
    // cycle from a later copy, then longer and shorter cycles.
    for repeats in [wanted.repeats, !wanted.repeats] {
        for &axis in &axes {
            let mut cycles: Vec<u32> = (1..=copies).collect();
            cycles.sort_by_key(|every| every.abs_diff(wanted.every));
            for every in cycles {
                let mut starts: Vec<u32> = (1..=copies).collect();
                starts.sort_by_key(|start| (*start < wanted.start, start.abs_diff(wanted.start)));
                for start in starts {
                    let candidate = Variation { axis, repeats, every, start, ..wanted };
                    if !taken(&candidate) {
                        return Some(candidate);
                    }
                }
            }
        }
    }
    None
}

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
