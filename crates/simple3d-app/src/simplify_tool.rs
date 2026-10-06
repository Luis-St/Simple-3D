//! The tool that drops detail from a mesh (issue 106).
//!
//! See [`Simplify`] for the settings and [`simple3d_geom::simplify`] for the algorithm.
//!
//! The preview is the actual result, computed on a thread and placed in the document (without an
//! undo step) while the window is open; Simplify keeps it and Cancel restores the original.
//! Simplification is judged by looking, so only the real surface will do. A wireframe of the new
//! triangulation can be drawn over it, and the rest of the scene follows the document's
//! [`PreviewViewport`](simple3d_core::scene::PreviewViewport). A non-modal
//! [in-place popup](crate::popup), so the shape can be orbited while tuning.

mod committed;
pub(crate) use committed::*;
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
    /// The mesh as it was when the tool opened. Every run starts from this rather than the preview,
    /// so errors do not compound and raising a number recovers detail.
    pub original: Arc<MeshData>,
    pub plan: Simplify,
    /// The preview in the document, and what it came to.
    pub shown: Option<Shown>,
    /// The run in flight; at most one, since only the newest result matters.
    pub job: Option<SimplifyJob>,
    /// Whether the result's triangles are drawn over the shape.
    pub wireframe: bool,
    /// The document evaluated without the preview, for the status bar's numbers.
    pub committed: Option<Committed>,
}

/// A finished run, as it stands in the document.
pub struct Shown {
    /// What it was computed for, so only a change of numbers starts another run.
    pub plan: Simplify,
    pub mesh: Arc<MeshData>,
    /// The furthest the surface moved, in millimetres.
    pub deviation: f64,
}

/// The popup's key, which also remembers where it was dragged.
const KEY: &str = "simplify-tool";

/// The window width: a label and its field, and no more, since it covers the model.
const WIDTH: f32 = 320.0;

/// A number field's width: enough for a length with its unit.
const FIELD_WIDTH: f32 = 90.0;

/// The percentage of the mesh to keep, 1 to 100.
const DETAIL: ParamKind = ParamKind::Count { min: 1, max: 100 };

/// How far the surface may move; zero collapses only free flat areas.
const DEVIATION: ParamKind = ParamKind::Length { min: 0.0 };

/// The corner angle's range; at a few degrees every facet is a corner.
const SHARP: ParamKind = ParamKind::Angle { min: 1.0, max: 179.0, wrap: false };

/// The most triangles the wireframe draws; past this nothing is drawn and the window says so.
const WIREFRAME_LIMIT: usize = 8_000;
