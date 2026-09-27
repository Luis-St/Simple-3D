//! How many variations a stage has room for, and the next free one (issue 79).

use super::*;
use std::collections::HashSet;

/// How many distinct `what` variations a stage of `copies` copies can hold: one per axis, stepping
/// and reach.
pub fn combinations(what: Vary, copies: u32) -> usize {
    let copies = copies.clamp(1, MAX_CYCLE) as usize;
    what.axes().len() * STEPS.len() * copies * copies
}

/// Whether the stage has room for another `what` variation unlike any it holds.
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

/// `wanted`, or if the stage has one like it, the nearest different one: another axis first, then
/// other copies. `None` when all are taken. Searched on demand, since the space can be huge but the
/// answer is nearly always found at once.
pub fn free_variation(stage: &Stage, wanted: Variation) -> Option<Variation> {
    let copies = (stage.copies() as u32).clamp(1, MAX_CYCLE);
    // Clamp reach to the stage's copies, since reaching beyond changes nothing.
    let wanted = Variation { every: wanted.every.min(copies), start: wanted.start.min(copies), ..wanted };
    let taken = |v: &Variation| stage.variations().iter().any(|held| held.combination() == v.combination());
    let axes: Vec<usize> =
        std::iter::once(wanted.axis).chain(wanted.what.axes().iter().copied().filter(|a| *a != wanted.axis)).collect();
    // The same copies along another axis, nearly always what was meant.
    if let Some(other) = axes.iter().map(|&axis| Variation { axis, ..wanted }).find(|v| !taken(v)) {
        return Some(other);
    }
    // Then other copies, outward from the asked-for ones.
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
