//! Custom patterns: stages combined into one rule.

use super::*;
use crate::primitive::Params;
use crate::xform::Xform;

/// Every parameter a custom rule is made of, which is what a saved kind holds
/// and what applying one writes.
pub fn custom_keys() -> Vec<&'static str> {
    let mut keys = vec!["stages"];
    for index in 0..MAX_STAGES {
        keys.extend(stage_keys(index));
    }
    keys
}

pub(crate) fn custom(params: &Params) -> Vec<Instance> {
    // Start with the shape itself, and let each stage repeat everything that
    // came before it. The outer transform is the later stage's, so "a row of
    // five, turned four times round Z" turns the whole row rather than each
    // copy where it stands.
    let mut out = vec![Instance::plain(Xform::IDENTITY)];
    for index in 0..stage_count(params) {
        let stage = stage(params, index);
        let mut next: Vec<Instance> = Vec::new();
        'fill: for outer in stage.instances() {
            for inner in &out {
                // The cap is checked while building rather than by truncating
                // afterwards, for the reason a grid checks it: four stages of
                // 512 multiply to more transforms than there is memory for.
                if next.len() >= MAX_INSTANCES {
                    break 'fill;
                }
                next.push(Instance {
                    xform: outer.xform.compose(&inner.xform),
                    // A reflection of a reflection points outward again.
                    mirrored: outer.mirrored != inner.mirrored,
                });
            }
        }
        out = next;
    }
    out
}

// -- laying a pattern out in the viewport (issue 67) -------------------------
//
// A pattern is a rule, and a rule is a handful of numbers -- but nobody lays a
// ring of bolt holes out by typing a radius. Each kind therefore offers a set
// of *grips*: points in the pattern's own frame that can be taken hold of in
// the viewport and dragged, each one writing exactly one of the numbers the
// property editor shows. They live here, beside the maths that places the
// copies, so the handle a user drags and the copy it sits on cannot disagree,
// and so the whole layout can be tested without a viewport.

/// What dragging a grip writes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Drive {
    /// A length. The distance the grip is dragged to, less `base` and divided
    /// by `per`, is the value -- so a grip that sits at the *last* copy divides
    /// by the number of gaps and writes the step between two.
    ///
    /// One key sets that parameter on its own; three set a run that can point
    /// anywhere, whose direction is kept and whose length becomes the value, so
    /// lengthening a run that steps diagonally keeps the diagonal rather than
    /// straightening it onto X.
    Length { keys: &'static [&'static str], base: f64, per: f64, min: f64 },
    /// A count: how many copies reach as far as the grip was dragged, at the
    /// spacing the pattern already has. This is what "lay one out" means for a
    /// straight run -- drag outward and copies follow the pointer.
    Count { key: &'static str, base: f64, per: f64 },
    /// An angle in degrees: the angle the grip was dragged round to, about
    /// `axis`. A grip that sits on the *second* copy divides by the one turn
    /// between it and the first, and so writes the turn per copy.
    Angle { key: &'static str, axis: usize, per: f64 },
}
