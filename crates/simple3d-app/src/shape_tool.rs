//! The tool that runs a function over a mesh's surface (issue 81): noise, ridges, cells, smoothing or
//! inflating. See [`Shaping`] for the settings and [`simple3d_geom::shaping`] for the functions.
//!
//! Previews like Simplify: the real result is computed on a thread and stands in the document while
//! the window is open (`crate::mesh_preview`), since a surface texture is judged by looking at it.
//! Apply keeps it as one undo step and Cancel puts the mesh back. Every run starts from the mesh as it
//! was when the tool opened, so scrubbing a number never compounds.

mod open;
mod preview;
mod run;
mod window;
pub(crate) use window::*;
mod controls;
pub(crate) use controls::*;
mod summary;
pub(crate) use summary::*;

use crate::worker::ShapeJob;
use simple3d_core::mesh_data::MeshData;
use simple3d_core::primitive::ParamKind;
use simple3d_core::scene::NodeId;
use simple3d_geom::shaping::Shaping;
use std::sync::Arc;

/// What the tool is working on while its window is open.
pub struct ShapeTool {
    pub target: NodeId,
    /// The mesh as it was when the tool opened, which every run starts from.
    pub original: Arc<MeshData>,
    pub plan: Shaping,
    /// The result standing in the document, and what it came to.
    pub shown: Option<Shown>,
    /// The run in flight; at most one, since only the newest result matters.
    pub job: Option<ShapeJob>,
}

/// A finished run, as it stands in the document.
pub struct Shown {
    /// What it was computed for, so only a change of numbers starts another run.
    pub plan: Shaping,
    pub mesh: Arc<MeshData>,
    /// Whether refining stopped at its triangle budget.
    pub capped: bool,
    /// The furthest any vertex moved, in millimetres.
    pub moved: f64,
}

/// The popup's key, which also remembers where it was dragged.
const KEY: &str = "shape-tool";

/// The window width: a label and its field, and no more, since it covers the model.
const WIDTH: f32 = 330.0;

/// A number field's width: enough for a length with its unit.
const FIELD_WIDTH: f32 = 90.0;

/// How far the surface moves; negative pushes in.
const AMOUNT: ParamKind = ParamKind::Length { min: f64::NEG_INFINITY };

/// A feature's size; below a hundredth of a millimetre nothing could show it.
const SCALE: ParamKind = ParamKind::Length { min: 0.01 };

/// Layers of finer detail; past eight they are finer than any mesh here.
const LAYERS: ParamKind = ParamKind::Count { min: 1, max: 8 };

/// Smoothing passes.
const PASSES: ParamKind = ParamKind::Count { min: 1, max: 100 };

/// Which pattern of a kind.
const SEED: ParamKind = ParamKind::Count { min: 1, max: 99_999 };

/// The longest edge refining leaves; finer than this runs into the triangle budget on anything big.
const EDGE: ParamKind = ParamKind::Length { min: 0.05 };

/// How much of the bottom is kept.
const BASE: ParamKind = ParamKind::Length { min: 0.0 };
