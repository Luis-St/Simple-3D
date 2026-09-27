//! What a split was asked to do, and reading stored plans back.

use super::*;
use crate::vec3::Vec3;
use serde::{Deserialize, Serialize};

/// Several tilings cut one after another, for multi-axis cuts with different cell shapes (issue 82).
/// Each pass cuts the previous pieces, so pieces are the intersection of all tilings and the order
/// does not matter.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct SplitPlan {
    pub passes: Vec<Tiling>,
}

impl Default for SplitPlan {
    fn default() -> SplitPlan {
        SplitPlan::of(Tiling::default())
    }
}

/// Read either a list of tilings or the older single-tiling object.
impl<'de> Deserialize<'de> for SplitPlan {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<SplitPlan, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Read {
            Many(Vec<Tiling>),
            One(Box<Tiling>),
        }
        Ok(match Read::deserialize(deserializer)? {
            Read::Many(passes) => SplitPlan { passes },
            Read::One(tiling) => SplitPlan::of(*tiling),
        })
    }
}

/// The most passes one split may have; each pass multiplies the piece count.
pub const MAX_PASSES: usize = 3;

impl SplitPlan {
    pub fn of(tiling: Tiling) -> SplitPlan {
        SplitPlan { passes: vec![tiling] }
    }

    /// The first pass, which names the summary.
    pub fn first(&self) -> Tiling {
        self.passes.first().copied().unwrap_or_default()
    }

    /// Why this plan cannot be cut, if so: an unbuildable pass or too many pieces in total.
    pub fn refusal(&self, bounds: (Vec3, Vec3)) -> Option<String> {
        if self.passes.is_empty() {
            return Some("There is nothing to cut with: add a cut.".to_string());
        }
        for tiling in &self.passes {
            // A pass is checked on its own first, so the message names the too-small cell.
            if let Some(why) = tiling.refusal(bounds) {
                return Some(why);
            }
        }
        let cells = self.cell_count(bounds);
        if cells > MAX_CELLS {
            return Some(format!(
                "The cuts come to {cells} cells between them, and {MAX_CELLS} is as many as one split may make. \
                 Use a bigger cell, or one cut fewer."
            ));
        }
        None
    }

    /// The arithmetic cell count: the product over passes.
    pub fn cell_count(&self, bounds: (Vec3, Vec3)) -> usize {
        self.passes.iter().fold(1usize, |total, tiling| total.saturating_mul(tiling.cell_count(bounds)))
    }

    /// How many cells could actually hold a piece of the shape, the number worth showing.
    pub fn planned(&self, bounds: (Vec3, Vec3)) -> usize {
        if self.refusal(bounds).is_some() {
            return 0;
        }
        self.passes.iter().fold(1usize, |total, tiling| total.saturating_mul(planned(tiling, bounds)))
    }

    /// How many cells will be tried, for progress: a running product over the passes.
    pub fn work(&self, bounds: (Vec3, Vec3)) -> usize {
        let mut total = 0usize;
        let mut running = 1usize;
        for tiling in &self.passes {
            running = running.saturating_mul(planned(tiling, bounds));
            total = total.saturating_add(running);
        }
        total
    }

    /// Every pass's cut lines for the preview, with the line cap shared between passes.
    pub fn preview_loops(&self, bounds: (Vec3, Vec3), limit: usize) -> Vec<Vec<Vec3>> {
        let each = limit / self.passes.len().max(1);
        self.passes.iter().flat_map(|tiling| preview_loops(tiling, bounds, each)).collect()
    }
}
