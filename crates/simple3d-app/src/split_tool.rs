//! The tool that cuts a shape into a pattern of smaller pieces (issue 82).
//!
//! It cuts a shape that is in a single piece: into squares, rectangles,
//! triangles or hexagons, running through the shape along an axis and
//! optionally cut into layers across it as well. A cut can be made more than
//! once over -- each cut on its own axis, in its own cell shape, applied to what
//! the last one left -- so a plate can be scored into a grid of blocks in one
//! gesture rather than by splitting a split. It ends in a
//! [`Body::Split`](simple3d_core::scene::Body::Split) standing where the shape
//! stood, holding the pieces and the shape itself -- so it is undone by one
//! Join back together, however long afterwards.
//!
//! The window is where the pattern is chosen, and the cells are drawn **over
//! the model itself** while they are being chosen: the one question a number of
//! millimetres cannot answer on its own is what it looks like against the thing
//! being cut, and the honest answer to that is the thing being cut. They are
//! drawn *by the renderer*, with the depth buffer, so a cut on the far side of
//! the shape is behind it -- a grid that shows through the solid it is drawn on
//! reads as lying in front of it, and which cells are on the face turned
//! towards you is most of what the picture is for. Nothing is
//! cut until Split is pressed, and the cutting itself happens on a thread --
//! see [`crate::worker::SplitJob`] -- because a hexagon tiling over a plate is
//! hundreds of booleans and an interface that stops answering is one nobody can
//! tell from a crashed one.
//!
//! It is an [in-place popup](crate::popup) rather than a dialog: it floats over
//! the viewport, is dragged around by its own title bar and rolls up out of the
//! way, and it does not stop the model underneath it being orbited, zoomed or
//! selected. That is what lets the viewport be the modelling area and the
//! preview area at once -- the cells are drawn in it, in world space, and the
//! way to see whether they fall where they should is to orbit the model with
//! the numbers still on screen. What the viewport does *under* the cells while
//! that is happening is the document's own
//! [`PreviewViewport`](simple3d_core::scene::PreviewViewport) setting, because
//! whether the grid, the axes or the rest of the scene is the nuisance depends
//! on what is being cut.
//!
//! Being non-modal also means the shape can change underneath the tool, so it
//! re-bakes what it is cutting whenever the evaluation moves on, and closes
//! itself if the shape goes away.

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
    /// The shape as it stands, baked in its own frame: what the cells are cut
    /// out of, what the estimate is counted over, and what the picture is drawn
    /// from.
    ///
    /// Re-baked whenever the evaluation moves on, because the window is not
    /// modal: the shape can be edited, hidden or moved while the tool is open,
    /// and a plan drawn over the shape as it *was* is a plan of cuts that will
    /// not fall there.
    pub mesh: Arc<Mesh>,
    pub bounds: (Vec3, Vec3),
    /// Where the baked frame stands in the world, so the cells can be drawn on
    /// the shape rather than beside it. The tiling is worked out in the shape's
    /// own frame -- that is the frame a cell size in millimetres means something
    /// in -- and this is the way back out to the viewport.
    pub placement: Xform,
    /// Which evaluation `mesh` was baked from, so the tool can tell when what
    /// it is drawing has gone stale.
    pub generation: u64,
    /// The cuts to make, in order: each one applied to the pieces the last left,
    /// so two of them on different axes make the blocks their grids come to
    /// between them.
    pub plan: SplitPlan,
}

impl SplitTool {
    /// Everything the preview drawn over the model depends on, for the key the
    /// viewport's cached image is rebuilt on. The loops themselves are not
    /// hashed: they are thousands of points, rebuilt from exactly these numbers.
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
        for point in [self.bounds.0, self.bounds.1, self.placement.t] {
            for number in [point.x, point.y, point.z] {
                number.to_bits().hash(hasher);
            }
        }
        for row in self.placement.m {
            for number in row {
                number.to_bits().hash(hasher);
            }
        }
    }
}

/// Identifies the popup, and is what remembers where it was dragged to.
const KEY: &str = "split-tool";

/// How wide the window is: enough for a labelled field and a plan of the cells
/// under it, and no wider. A popup lives over the model, so every pixel of it
/// is a pixel of the thing being cut that cannot be seen.
const WIDTH: f32 = 340.0;

/// A cell size: never negative, and never quite zero -- a cell of no size is
/// refused by the tiling itself, and a field that can reach it only wastes the
/// press that finds out.
const SIZE: ParamKind = ParamKind::Length { min: 0.0 };

/// How wide a number field is: enough for a length with its unit on it, and no
/// wider -- a popup lives over the model, and every pixel of it is a pixel of
/// the thing being cut that cannot be seen.
const FIELD_WIDTH: f32 = 90.0;

/// The most cell outlines the preview will draw in a frame.
///
/// A split may ask for ten thousand cells, and the preview draws them at both
/// ends of the run and at every layer between -- which is a number of line
/// loops that costs more per frame than the picture is worth. Past this the
/// preview is the part of the grid that was laid down first, which is the far
/// end and then the near one.
const PREVIEW_LOOPS: usize = 3_000;
