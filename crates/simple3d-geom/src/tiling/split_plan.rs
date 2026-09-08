//! What the user asked a split to do, and how a stored one is read back.

use super::*;
use crate::vec3::Vec3;
use serde::{Deserialize, Serialize};

/// Several tilings, cut one after another, so a shape can be cut on more than
/// one axis at once and with a different cell shape on each (issue 82).
///
/// One tiling answers "squares through Z". Two answer "squares through Z, then
/// slabs through X", which is the pattern a plate is scored into a grid of
/// blocks by, and what neither a single tiling nor two separate splits can say:
/// splitting a split cuts the *pieces* of one into a second collection, and the
/// way back is then two joins deep.
///
/// The passes are applied in order, each to the pieces the last one left, so
/// the pieces are the cells of every tiling intersected. That is also why the
/// order does not change the answer -- intersection does not care -- and why
/// nothing here has to reason about how two lattices meet.
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

/// Read either a list of tilings or a single one.
///
/// A split used to be cut by exactly one tiling and wrote it as an object. A
/// file or a settings file written then still says what was done, and there is
/// no reason to lose it over a pair of brackets.
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

/// The most passes one split may be cut by.
///
/// Three is not a technical limit -- the pieces of any pass can be cut again --
/// but each pass multiplies the pieces the last one made, so a fourth is a
/// number nobody typed and a wait nobody asked for. Two is what "on more than
/// one axis" means; the third is there for the plate that is also cut into
/// layers a different way.
pub const MAX_PASSES: usize = 3;

impl SplitPlan {
    pub fn of(tiling: Tiling) -> SplitPlan {
        SplitPlan { passes: vec![tiling] }
    }

    /// The first pass, which is the whole plan for the ordinary one-pass split
    /// and the one a summary is named after.
    pub fn first(&self) -> Tiling {
        self.passes.first().copied().unwrap_or_default()
    }

    /// Whether this is a plan that can be cut at all: every pass buildable on
    /// its own, and the pieces they come to between them a number a person can
    /// still find in the outliner.
    pub fn refusal(&self, bounds: (Vec3, Vec3)) -> Option<String> {
        if self.passes.is_empty() {
            return Some("There is nothing to cut with: add a cut.".to_string());
        }
        for tiling in &self.passes {
            // A single pass is refused by its own count first, so the message
            // names the cell that is too small rather than the total.
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

    /// How many cells the passes lay over a shape of these bounds between them,
    /// as arithmetic -- the product, because every cell of one pass is cut by
    /// every cell of the next.
    pub fn cell_count(&self, bounds: (Vec3, Vec3)) -> usize {
        self.passes.iter().fold(1usize, |total, tiling| total.saturating_mul(tiling.cell_count(bounds)))
    }

    /// How many cells the passes actually lay over the shape -- the ones that
    /// could hold a piece of it, which is the number worth showing.
    pub fn planned(&self, bounds: (Vec3, Vec3)) -> usize {
        if self.refusal(bounds).is_some() {
            return 0;
        }
        self.passes.iter().fold(1usize, |total, tiling| total.saturating_mul(planned(tiling, bounds)))
    }

    /// How many cells will be *tried*, which is what progress is measured
    /// against: the first pass over the shape, then the second over each piece
    /// the first left, and so on. Every pass but the first is counted against
    /// the pieces before it, which is why this is a running product rather than
    /// the last one.
    pub fn work(&self, bounds: (Vec3, Vec3)) -> usize {
        let mut total = 0usize;
        let mut running = 1usize;
        for tiling in &self.passes {
            running = running.saturating_mul(planned(tiling, bounds));
            total = total.saturating_add(running);
        }
        total
    }

    /// Where every pass's cuts will fall, for drawing them over the model. The
    /// cap is shared out between the passes, so a second cut cannot be squeezed
    /// off the picture by the first one filling it.
    pub fn preview_loops(&self, bounds: (Vec3, Vec3), limit: usize) -> Vec<Vec<Vec3>> {
        let each = limit / self.passes.len().max(1);
        self.passes.iter().flat_map(|tiling| preview_loops(tiling, bounds, each)).collect()
    }
}
