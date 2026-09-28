//! A boolean computed per pixel, for the frames of a drag that changes it.
//!
//! Evaluating the boolean in the kernel is far too slow for a frame, so only its appearance is
//! computed: surfaces are depth-peeled layer by layer, each shape's faces in front of a layer are
//! counted (odd means inside), and the expression is evaluated with the layer's own shape counted
//! both inside and outside. Where the answers differ the point is on the result's surface and is
//! drawn (`App::live_csg`).
//!
//! A section is one more shape: the kept half-space as a box, intersected with the whole.
//!
//! Shapes must be closed. Faces two shapes share lie on the layer together (`ON_LAYER`) and flip
//! together, so equal boxes cut cleanly; the real evaluation still happens on release.

use super::*;
use crate::render::{CsgPreview, Renderable, Request};
use eframe::glow::{self, HasContext};
use resident::{set_forward, set_projection, Placing};
use simple3d_geom::section::Plane;
use simple3d_geom::{Mesh, Vec3};

mod half_space;
mod inherit;
mod peel;

/// The most layers a frame peels, bounding pathological shapes; peeling stops at the first empty layer.
const MAX_LAYERS: usize = 32;

/// The most shapes a boolean is drawn from, including the section: one bit each of the resolve mask.
pub(crate) const MAX_SHAPES: usize = 128;

/// The fewest layers a frame peels when not querying as it goes.
const MIN_LAYERS: usize = 4;

/// How near in depth a face must be to a layer to lie on it rather than in front or behind. Faces
/// two shapes share, like two equal boxes' tops, meet the layer together; a strict depth test
/// counted the other one in front or not by rounding, pixel by pixel, and speckled the cut away.
const ON_LAYER: f32 = 4.0e-6;

/// Frame-sized render targets for the passes.
pub(crate) struct CsgTargets {
    width: usize,
    height: usize,
    /// Two layers' depths, one peeled while the other is read.
    depth: [glow::Texture; 2],
    /// Which shape each pixel of the layer is on, and its colour there.
    leaf: glow::Texture,
    colour: glow::Texture,
    /// The counts, four shapes per texture, for one group of up to 32 shapes.
    counts: [glow::Texture; 8],
    /// Inside bits per shape: 32 per channel, one channel per group (`CSG_PACK_FRAGMENT`).
    inside: glow::Texture,
    /// The same for the shapes with a face on the layer, which flip there.
    on: glow::Texture,
    peel: [glow::Framebuffer; 2],
    /// Without depth: the count compares with the layer's depth itself (`CSG_COUNT_FRAGMENT`).
    count: glow::Framebuffer,
    pack: glow::Framebuffer,
    /// One occlusion query per layer: whether it had anything in it. See [`LayerPlan`].
    queries: Vec<glow::Query>,
    /// How many layers the last frame drew; `None` when there was no last frame.
    last: std::cell::Cell<Option<usize>>,
}

/// How many layers a frame peels, and whether it queries after each one.
///
/// Querying after each layer stalls CPU and GPU alternately, up to 32 times a frame. Instead a
/// frame draws as many layers as the last needed plus two, and reads the results a frame late;
/// if every layer was used, the next frame doubles. Only a frame with no history (the first of a
/// drag, or after a resize) queries as it goes.
enum LayerPlan {
    Asking,
    Fixed(usize),
}

impl CsgTargets {
    unsafe fn new(gl: &glow::Context, width: usize, height: usize) -> Result<CsgTargets, String> {
        let texture = |internal: u32, format: u32, kind: u32| -> Result<glow::Texture, String> {
            let texture = gl.create_texture()?;
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            clamp_texture(gl, glow::NEAREST, glow::NEAREST);
            let (w, h) = (width as i32, height as i32);
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                internal as i32,
                w,
                h,
                0,
                format,
                kind,
                glow::PixelUnpackData::Slice(None),
            );
            Ok(texture)
        };
        let depth_texture = || texture(glow::DEPTH_COMPONENT32F, glow::DEPTH_COMPONENT, glow::FLOAT);
        let depth = [depth_texture()?, depth_texture()?];
        // The shape in the low byte, the surface normal packed above it for the edge pass.
        let leaf = texture(glow::R32UI, glow::RED_INTEGER, glow::UNSIGNED_INT)?;
        let colour = texture(glow::RGBA8, glow::RGBA, glow::UNSIGNED_BYTE)?;
        let mut counts = Vec::with_capacity(8);
        for _ in 0..8 {
            counts.push(texture(glow::RGBA8, glow::RGBA, glow::UNSIGNED_BYTE)?);
        }
        let counts: [glow::Texture; 8] = counts.try_into().expect("eight made");
        let inside = texture(glow::RGBA32UI, glow::RGBA_INTEGER, glow::UNSIGNED_INT)?;
        let on = texture(glow::RGBA32UI, glow::RGBA_INTEGER, glow::UNSIGNED_INT)?;
        gl.bind_texture(glow::TEXTURE_2D, None);

        let framebuffer = |depth: Option<glow::Texture>, colours: &[glow::Texture]| -> Result<glow::Framebuffer, String> {
            let framebuffer = gl.create_framebuffer()?;
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
            if let Some(depth) = depth {
                gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::DEPTH_ATTACHMENT, glow::TEXTURE_2D, Some(depth), 0);
            }
            for (index, &texture) in colours.iter().enumerate() {
                let attachment = glow::COLOR_ATTACHMENT0 + index as u32;
                gl.framebuffer_texture_2d(glow::FRAMEBUFFER, attachment, glow::TEXTURE_2D, Some(texture), 0);
            }
            let buffers: Vec<u32> = (0..colours.len() as u32).map(|index| glow::COLOR_ATTACHMENT0 + index).collect();
            gl.draw_buffers(&buffers);
            let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
            if status != glow::FRAMEBUFFER_COMPLETE {
                return Err(format!("the boolean preview's target is not usable (status {status:#x})"));
            }
            Ok(framebuffer)
        };
        let peel = [framebuffer(Some(depth[0]), &[colour, leaf])?, framebuffer(Some(depth[1]), &[colour, leaf])?];
        let count = framebuffer(None, &counts)?;
        // A full-frame pass with no depth test.
        let pack = framebuffer(None, &[inside, on])?;
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        let queries = (0..MAX_LAYERS).map(|_| gl.create_query()).collect::<Result<Vec<_>, _>>()?;
        Ok(CsgTargets {
            width,
            height,
            depth,
            leaf,
            colour,
            counts,
            inside,
            on,
            peel,
            count,
            pack,
            queries,
            last: std::cell::Cell::new(None),
        })
    }

    unsafe fn destroy(&self, gl: &glow::Context) {
        for framebuffer in self.peel.iter().chain([&self.count, &self.pack]) {
            gl.delete_framebuffer(*framebuffer);
        }
        let textures = self.depth.iter().chain([&self.leaf, &self.colour, &self.inside, &self.on]).chain(&self.counts);
        for texture in textures {
            gl.delete_texture(*texture);
        }
        for query in &self.queries {
            gl.delete_query(*query);
        }
    }

    /// What this frame peels, from what the last one's layers held (see [`LayerPlan`]).
    unsafe fn plan(&self, gl: &glow::Context) -> LayerPlan {
        let Some(drawn) = self.last.get() else { return LayerPlan::Asking };
        // Queries finish in order, so the last one being available means all are.
        if gl.get_query_parameter_u32(self.queries[drawn - 1], glow::QUERY_RESULT_AVAILABLE) == 0 {
            return LayerPlan::Fixed(drawn);
        }
        let held =
            (0..drawn).position(|layer| gl.get_query_parameter_u32(self.queries[layer], glow::QUERY_RESULT) == 0);
        LayerPlan::Fixed(match held {
            Some(empty) => (empty + 2).clamp(MIN_LAYERS, MAX_LAYERS),
            None => (drawn * 2).min(MAX_LAYERS),
        })
    }

    /// Forget the last boolean's depth, for a frame that draws none.
    pub(super) fn forget(&self) {
        self.last.set(None);
    }
}

impl Gpu {
    /// Size the preview's targets to the frame, if it has a boolean to draw.
    pub(super) unsafe fn ensure_csg_targets(
        &mut self,
        gl: &glow::Context,
        width: usize,
        height: usize,
    ) -> Result<(), String> {
        if let Some(targets) = &self.csg_targets {
            if targets.width == width && targets.height == height {
                return Ok(());
            }
            targets.destroy(gl);
            self.csg_targets = None;
        }
        self.csg_targets = Some(CsgTargets::new(gl, width, height)?);
        Ok(())
    }

    /// The boolean's edges over what `draw_csg` resolved: each shape's feature edges, kept where the
    /// pixel's surface is that shape's (`CSG_EDGE_FRAGMENT`). Needed because the scene's own lines
    /// hide the dragged part.
    ///
    /// Expects the model pass's state after the lines, and leaves it so.
    pub(super) unsafe fn draw_csg_edges(
        &self,
        gl: &glow::Context,
        request: &Request<'_>,
        csg: &CsgPreview<'_>,
        colour: crate::raster::Rgba,
        viewport: [f32; 2],
        depth: [f32; 2],
    ) {
        let Some(scene) = &self.target else { return };
        let program = &self.csg_edges;
        // Detached from the framebuffer so the texture is never read and written at once.
        gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT2, glow::TEXTURE_2D, None, 0);
        gl.use_program(Some(program.program));
        gl.enable(glow::CLIP_DISTANCE0);
        set2(gl, program, "u_viewport", viewport);
        set2(gl, program, "u_depth", depth);
        set4(gl, program, "u_colour", &as_float(colour));
        set_f32(gl, program, "u_bias", crate::render::EDGE_BIAS);
        set_i32(gl, program, "u_tagged", 0);
        set_u32(gl, program, "u_tag", csg.tag as u32);
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, Some(scene.done));
        set_i32(gl, program, "u_done", 0);
        for (index, (leaf, moved)) in csg.leaves.iter().enumerate() {
            let Some(resident) = self.resident.get(&leaf.id) else { continue };
            let Some(edges) = &resident.edges else { continue };
            if edges.count == 0 {
                continue;
            }
            set_projection(gl, program, resident, &request.view, &request.section, &Placing::moved(*moved));
            set_f32(gl, program, "u_leaf_code", (index + 1) as f32 / 255.0);
            gl.bind_vertex_array(Some(edges.array));
            gl.draw_elements(glow::LINES, edges.count, glow::UNSIGNED_INT, 0);
        }
        gl.bind_texture(glow::TEXTURE_2D, None);
        gl.disable(glow::CLIP_DISTANCE0);
        gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT2, glow::TEXTURE_2D, Some(scene.done), 0);
        gl.bind_vertex_array(Some(self.buffer.array));
    }
}
