//! The tool that cuts a shape into a pattern of smaller pieces (issue 82).
//!
//! Squares, rectangles, triangles or hexagons along an axis, optionally in layers; several cuts
//! can be chained. The result is a [`Body::Split`](simple3d_core::scene::Body::Split) holding the
//! pieces and the original shape, undone by Join.
//!
//! The cells are drawn over the model by the renderer, with depth, while being chosen. Cutting
//! happens on a thread ([`crate::worker::SplitJob`]), since a hexagon tiling is hundreds of
//! booleans. A non-modal [in-place popup](crate::popup), with the viewport underneath governed by
//! [`PreviewViewport`](simple3d_core::scene::PreviewViewport). The shape is re-baked when the
//! evaluation moves on, and the tool closes if the shape goes away.

mod open;
mod run;
mod window;
pub(crate) use window::*;
mod controls;
pub(crate) use controls::*;
mod fields;
pub(crate) use fields::*;
mod summary;
pub(crate) use summary::*;

use simple3d_core::primitive::ParamKind;
use simple3d_core::scene::NodeId;
use simple3d_core::xform::Xform;
use simple3d_geom::tiling::SplitPlan;
use simple3d_geom::{Mesh, Vec3};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// What the tool is working on while its window is open.
pub struct SplitTool {
    pub target: NodeId,
    /// The shape baked in its own frame, re-baked whenever the evaluation moves on, since the shape
    /// can change while the non-modal tool is open.
    pub mesh: Arc<Mesh>,
    pub bounds: (Vec3, Vec3),
    /// The baked frame's world placement, so cells computed in the shape's frame draw on it.
    pub placement: Xform,
    /// The evaluation `mesh` was baked from, to detect staleness.
    pub generation: u64,
    /// The cuts to make, each applied to the pieces the previous one left.
    pub plan: SplitPlan,
}

impl SplitTool {
    /// Hash what the preview depends on into the viewport's image key; the loops follow from these.
    pub(crate) fn hash_preview<H: Hasher>(&self, hasher: &mut H) {
        self.target.hash(hasher);
        self.generation.hash(hasher);
        for tiling in &self.plan.passes {
            tiling.kind.hash(hasher);
            tiling.axis.hash(hasher);
            for number in [tiling.size, tiling.depth, tiling.angle, tiling.layer, tiling.offset[0], tiling.offset[1]] {
                number.to_bits().hash(hasher);
            }
        }
        for point in [self.bounds.0, self.bounds.1] {
            for number in [point.x, point.y, point.z] {
                number.to_bits().hash(hasher);
            }
        }
        self.placement.hash_bits(hasher);
    }
}

/// The popup's key, which also remembers where it was dragged.
const KEY: &str = "split-tool";

/// The window width: a labelled field and the cell plan, no wider, since it covers the model.
const WIDTH: f32 = 340.0;

/// A cell size; zero is refused by the tiling itself.
const SIZE: ParamKind = ParamKind::Length { min: 0.0 };

/// A number field's width: enough for a length with its unit.
const FIELD_WIDTH: f32 = 90.0;

/// The most cell outlines drawn per frame; past this only the first-laid part of the grid is shown.
const PREVIEW_LOOPS: usize = 3_000;
