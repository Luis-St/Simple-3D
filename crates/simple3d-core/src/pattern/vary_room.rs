//! How many variations a stage has room for, and the next free one (issue 79).

use super::*;
use std::collections::HashSet;

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
