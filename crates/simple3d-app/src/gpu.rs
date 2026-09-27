//! The viewport drawn through OpenGL, as an alternative to `raster.rs`.
//!
//! The same scene by the same rules, but everything derived from meshes is computed on the card:
//! meshes stay resident (`resident.rs`), and outlines, plane marks, caps and cut edges are found
//! there; grid, axes and previews are shaders too (`ground.rs`). A frame uploads only a camera.
//! The two renderers share rules, not code; the software one is the fallback without a GPU.
//!
//! Dragged bodies are moved on the card (`render::Live`), and booleans affected by a drag are
//! computed per pixel (`csg.rs`), so drags do not re-evaluate. Visibility queries are answered
//! from the drawn depth (`depth.rs`).
//!
//! The pictures are alike, not identical: GPU line rasterisation and batching differ slightly,
//! but depth test, bias and the axis see-through rule are reproduced. Sections through open
//! meshes differ: the CPU leaves unchainable cuts uncapped, the GPU's winding count caps them.
//!
//! This path may fail: every entry point returns a `Result`, and the viewport falls back to the
//! software renderer with the driver's message (spec section 2.7, acceptance criterion 19).

mod shader;
pub(crate) use shader::*;
mod program;
pub(crate) use program::*;
mod target;
pub(crate) use target::*;
mod passes;
pub(crate) use passes::*;
mod buffers;
mod csg;
mod depth;
mod draw;
mod ground;
mod render;
mod resident;
pub(crate) use buffers::*;

use eframe::glow;
use std::sync::Arc;

/// The fraction of the scene's depth extent left free at each end, so a biased line at the very
/// front or back still lands in the buffer.
const DEPTH_MARGIN: f32 = 0.05;

pub struct Gpu {
    gl: Arc<glow::Context>,
    solid: Program,
    axis: Program,
    grid: Program,
    background: Program,
    /// A resident mesh's faces and lines (see `resident.rs`).
    faces: Program,
    lines: Program,
    /// A selected body's outline, and where planes cross a mesh.
    outline: Program,
    crossing: Program,
    /// A boolean drawn per pixel while a drag changes it (see `csg.rs`).
    csg_peel: Program,
    csg_count: Program,
    csg_pack: Program,
    csg_resolve: Program,
    csg_edges: Program,
    csg_targets: Option<csg::CsgTargets>,
    /// Each section as a shape with its cutting operation, and the hash they were made from.
    csg_half: Option<(u64, Vec<(crate::render::Renderable, i32)>)>,
    /// The texel width of the mesh topology tables (see `resident::table`).
    table_width: usize,
    /// The meshes kept on the card, by `Renderable::id`.
    resident: std::collections::HashMap<u64, resident::Resident>,
    /// A tool's preview loops as lines, with their source hash (see `ground::refresh_preview`).
    preview: Option<(u64, crate::render::Renderable)>,
    /// The offscreen target, remade when the viewport size changes.
    target: Option<Target>,
    /// The texture the finished frame lands in, reallocated on resize rather than replaced, since
    /// egui is told its id only once.
    colour: Option<glow::Texture>,
    /// The vertex buffer everything is drawn from, reused between frames.
    buffer: Buffers,
    /// What the grid quad and the axis arms are drawn from.
    ground: GroundBuffers,
    /// The faces' depth, read back for queries about the picture (see `depth.rs`).
    depth: depth::DepthReadback,
    /// egui's id for the colour texture, registered once and valid for the application's life.
    pub texture_id: Option<egui::TextureId>,
}
