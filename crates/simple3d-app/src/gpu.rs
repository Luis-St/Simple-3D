//! The viewport drawn through OpenGL, as an alternative to `raster.rs`.
//!
//! It draws the *same* scene, from the same prepared primitives: `render.rs`
//! projects, culls, shades and works out the axis rule on the CPU, and this
//! module only turns the result into pixels. Nothing about what the picture
//! shows is decided here, which is what keeps the two engines from drifting
//! apart as the renderer is worked on.
//!
//! The two pictures are alike, not identical, and deliberately so. A GPU draws
//! a line by rasterizing it, where `raster.rs` walks it pixel by pixel, so a
//! one-pixel line lands slightly differently; and primitives are batched by
//! kind rather than issued one at a time, which reorders the few draws that
//! depth does not already separate. Everything that decides *what* is visible
//! -- the depth test, the bias, and the rule that lets an origin axis be seen
//! through the solid it is arriving at -- is reproduced exactly.
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
mod draw;
mod render;
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
    background: Program,
    /// The offscreen target, remade whenever the viewport's size changes.
    target: Option<Target>,
    /// The texture the finished frame lands in. Made once and reallocated on a
    /// resize rather than replaced, because egui is told its id exactly once
    /// and a new texture every frame would leak one every frame.
    colour: Option<glow::Texture>,
    /// The vertex buffer everything is drawn from, reused between frames.
    buffer: Buffers,
    /// What egui knows the colour texture as. Registered once: the texture
    /// object is kept and redrawn into, so the id stays good for the life of
    /// the application and no texture is leaked per frame.
    pub texture_id: Option<egui::TextureId>,
}
