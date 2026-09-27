//! Custom patterns: stages combined into one rule.

use super::*;
use crate::primitive::Params;
use crate::xform::Xform;

/// Every custom rule parameter besides its stages' variations.
pub fn custom_keys() -> Vec<&'static str> {
    let mut keys = vec!["stages"];
    for index in 0..MAX_STAGES {
        keys.extend(stage_keys(index));
    }
    keys
}

/// Every parameter of the rule in `params`, variations included: what a saved kind holds.
pub fn rule_keys(params: &Params) -> Vec<String> {
    let mut keys: Vec<String> = custom_keys().into_iter().map(str::to_string).collect();
    for index in 0..MAX_STAGES {
        keys.extend(variation_keys(index, variation_count(params, index)));
    }
    keys
}

/// Whether a key belongs to a custom rule.
pub fn is_custom_key(key: &str) -> bool {
    key == "stages" || stage_of(key).is_some()
}

/// Lay out a stack of stages: the copies every pattern kind ends up as.
pub(crate) fn lay_out(stages: &[Stage]) -> Vec<Instance> {
    // Each stage repeats everything before it, with the later stage's transform outside, so a row
    // turned four times turns as a whole.
    let mut out = vec![Instance::plain(Xform::IDENTITY)];
    for stage in stages {
        let mut next: Vec<Instance> = Vec::new();
        'fill: for outer in stage.instances() {
            for inner in &out {
                // Checked while building, since four stages of 512 exceed memory.
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
// Each kind offers grips in its own frame that write one number each when dragged. They live
// beside the placement maths so handles and copies cannot disagree.

/// What dragging a grip writes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Drive {
    /// A length: the dragged distance less `base`, over `per` (a last-copy grip divides by the gap
    /// count). Three keys set a run whose direction is kept and length changed.
    Length { keys: &'static [&'static str], base: f64, per: f64, min: f64 },
    /// A count: how many copies reach the grip at the current spacing.
    Count { key: &'static str, base: f64, per: f64 },
    /// An angle in degrees about `axis`; a second-copy grip divides by one turn, giving turn per copy.
    Angle { key: &'static str, axis: usize, per: f64 },
}
