//! The viewport drawn through OpenGL, as an alternative to `raster.rs`.
//!
//! It draws the *same* scene by the same rules, but everything that comes off
//! a mesh is worked out on the card: the meshes are kept there (`resident.rs`)
//! and projected, culled and shaded by the shaders, and the selection's
//! silhouette, the plane marks, the section's cap and the line round the cut
//! are found there too, from the mesh as it was uploaded. A frame hands the
//! card a camera and nothing per triangle. The grid, the axes with their rule
//! and a tool's preview are drawn by shaders as well (`ground.rs`), so nothing
//! of the frame is prepared on the CPU: `render.rs`'s steps are the software
//! renderer's alone. That renderer is the fallback for a machine without a
//! usable GPU, and this one is not held back to share its code paths; the two
//! share the rules -- where an axis stands, which grid levels show, what an
//! outline is -- and not the work.
//!
//! A body being dragged is moved on the card as well: its stretch of the
//! scene is left out and its own renderable is drawn with the drag's
//! transform (`render::Live`), so a drag that only moves, turns or scales a
//! body draws every frame without the scene being evaluated, and evaluates it
//! once when the body is let go.
//!
//! The two pictures are alike, not identical, and deliberately so. A GPU draws
//! a line by rasterizing it, where `raster.rs` walks it pixel by pixel, so a
//! one-pixel line lands slightly differently; and primitives are batched by
//! kind rather than issued one at a time, which reorders the few draws that
//! depth does not already separate. Everything that decides *what* is visible
//! -- the depth test, the bias, and the rule that lets an origin axis be seen
//! through the solid it is arriving at -- is reproduced. The one place the
//! rules differ is a section through a mesh that does not close: the CPU
//! leaves a cut it cannot chain into outlines uncapped, where the card, which
//! counts windings instead of chaining, caps whatever the open surface winds
//! round.
//!
//! There is no shader to fail to compile on the CPU path, so this one is
//! allowed to fail: every entry point returns a `Result`, and the viewport
//! falls back to the software renderer with the driver's own message rather
//! than showing nothing (spec section 2.7, acceptance criterion 19).

mod shader;
pub(crate) use shader::*;
mod program;
pub(crate) use program::*;
mod target;
pub(crate) use target::*;
mod passes;
pub(crate) use passes::*;
mod buffers;
mod depth;
mod draw;
mod ground;
mod render;
mod resident;
pub(crate) use buffers::*;

use eframe::glow;
use std::sync::Arc;

/// The depth range the scene is mapped into, as a fraction of its own extent
/// left free at each end. The depth *bias* a line gets is added to its key
/// before the mapping, so the margin has to be wide enough that a biased line
/// at the very front or back of the scene still lands inside the buffer.
const DEPTH_MARGIN: f32 = 0.05;

pub struct Gpu {
    gl: Arc<glow::Context>,
    solid: Program,
    axis: Program,
    grid: Program,
    background: Program,
    /// A resident mesh's faces and its lines -- see `resident.rs`.
    faces: Program,
    lines: Program,
    /// A selected body's outline, and where planes cross a mesh.
    outline: Program,
    crossing: Program,
    /// How many texels wide the tables a geometry stage reads a mesh's
    /// topology from are laid out -- see `resident::table`.
    table_width: usize,
    /// The meshes kept on the card, by `Renderable::id`.
    resident: std::collections::HashMap<u64, resident::Resident>,
    /// A tool's preview loops as lines, with the hash of the loops they were
    /// made from -- see `ground::refresh_preview`.
    preview: Option<(u64, crate::render::Renderable)>,
    /// The offscreen target, remade whenever the viewport's size changes.
    target: Option<Target>,
    /// The texture the finished frame lands in. Made once and reallocated on a
    /// resize rather than replaced, because egui is told its id exactly once
    /// and a new texture every frame would leak one every frame.
    colour: Option<glow::Texture>,
    /// The vertex buffer everything is drawn from, reused between frames.
    buffer: Buffers,
    /// What the grid's quad and the axes' arms are drawn from.
    ground: GroundBuffers,
    /// The faces' depth, read back for the questions asked about the
    /// picture -- see `depth.rs`.
    depth: depth::DepthReadback,
    /// What egui knows the colour texture as. Registered once: the texture
    /// object is kept and redrawn into, so the id stays good for the life of
    /// the application and no texture is leaked per frame.
    pub texture_id: Option<egui::TextureId>,
}
