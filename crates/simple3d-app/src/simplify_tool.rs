//! The tool that drops detail from a mesh (issue 106).
//!
//! A mesh that was imported or scanned is often far finer than the job it is
//! wanted for. This is where that is taken back: a percentage of the triangles
//! to keep, a distance the surface may not move, and three features that are
//! not to be given up -- see [`Simplify`] for what each one means and
//! [`simple3d_geom::simplify`] for how the mesh is actually reduced.
//!
//! ## The preview is the answer
//!
//! Nothing here computes a picture of what the result would be like. It
//! computes *the result*, on a thread, and puts it in the document while the
//! window is open: the shape in the viewport is the simplified mesh, not an
//! impression of one, and pressing Simplify keeps what is already on screen.
//! Cancel puts the mesh that was there back.
//!
//! That is worth the bookkeeping it costs -- an original to hold on to, a
//! document that is written to without an undo step until the tool is done --
//! because simplification is judged by *looking*. What a percentage does to a
//! shape cannot be read off the number: 20% of a scanned bracket is the same
//! bracket, and 20% of a fillet is a chamfer. Anything less than the real
//! surface, drawn with the real shading at the real distance, answers a
//! different question from the one being asked.
//!
//! Two things are drawn over it. The new triangulation, as a wireframe on the
//! shape (a change of a few thousand triangles usually shows in the
//! triangulation long before it shows in the silhouette), and, through the
//! document's own
//! [`PreviewViewport`](simple3d_core::scene::PreviewViewport) setting, as much
//! or as little of the rest of the scene as is useful -- the same option the
//! pattern and split tools preview through.
//!
//! It is an [in-place popup](crate::popup), non-modal like the rest, so the
//! model underneath can be orbited and zoomed while the numbers are being
//! turned. That is the whole point: the way to judge a simplification is to
//! turn the shape round and look at it.

mod open;
mod preview;
pub(crate) use preview::*;
mod run;
mod window;
pub(crate) use window::*;
mod controls;
pub(crate) use controls::*;
mod fields;
pub(crate) use fields::*;
mod summary;
pub(crate) use summary::*;

use crate::worker::SimplifyJob;
use simple3d_core::mesh_data::MeshData;
use simple3d_core::primitive::ParamKind;
use simple3d_core::scene::NodeId;
use simple3d_geom::simplify::Simplify;
use std::sync::Arc;

/// What the tool is working on while its window is open.
pub struct SimplifyTool {
    pub target: NodeId,
    /// The mesh as the document held it when the tool opened.
    ///
    /// Every run is computed from this rather than from whatever is currently
    /// on the node, because what is currently on the node is usually the last
    /// preview: simplifying a simplification compounds the error, and turning a
    /// number back up would then never recover the detail it gave away.
    pub original: Arc<MeshData>,
    pub plan: Simplify,
    /// The preview standing in the document, and what it came to.
    pub shown: Option<Shown>,
    /// The run in flight. At most one: a scrub asks for a new result on every
    /// frame it moves, and what is wanted is the newest of those, not each of
    /// them.
    pub job: Option<SimplifyJob>,
    /// Whether the triangles of the result are drawn over the shape.
    pub wireframe: bool,
}

/// A finished run, as it stands in the document.
pub struct Shown {
    /// What it was computed for, so a change to any number starts another run
    /// and nothing else does.
    pub plan: Simplify,
    pub mesh: Arc<MeshData>,
    /// The furthest the surface was moved, in millimetres. An upper bound: the
    /// real surface is at least this close.
    pub deviation: f64,
}

/// Identifies the popup, and is what remembers where it was dragged to.
const KEY: &str = "simplify-tool";

/// How wide the window is: a label, its field, and no more. A popup lives over
/// the model, so every pixel of it is a pixel of the thing being simplified
/// that cannot be seen.
const WIDTH: f32 = 320.0;

/// How wide a number field is: enough for a length with its unit on it.
const FIELD_WIDTH: f32 = 90.0;

/// How much of the mesh to keep, as a percentage. Never zero -- a mesh of no
/// triangles is not a simplified shape -- and never more than all of it.
const DETAIL: ParamKind = ParamKind::Count { min: 1, max: 100 };

/// How far the surface may move. Zero is allowed and means exactly what it
/// says: collapse only what costs nothing, which is the flat faces a fine
/// tessellation wasted its triangles on.
const DEVIATION: ParamKind = ParamKind::Length { min: 0.0 };

/// How sharp a crease has to be to be a corner. Below a few degrees every
/// facet of a curve is a corner and there is nothing left to simplify; at 180
/// nothing is.
const SHARP: ParamKind = ParamKind::Angle { min: 1.0, max: 179.0, wrap: false };

/// The most triangles the wireframe will draw.
///
/// Past this it is not drawn at all, and the window says so. A part of a
/// wireframe is not a lighter wireframe -- it is a picture of a different mesh
/// -- and forty thousand triangles of lines over the model is a frame that
/// takes longer to draw than the simplification took to compute.
const WIREFRAME_LIMIT: usize = 8_000;
