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

use crate::raster::{Rgba, Vertex};
use crate::render::{AxisStep, Prepared, Request, Step};
use eframe::glow::{self, HasContext};
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

struct Program {
    program: glow::Program,
    uniforms: std::collections::HashMap<String, glow::UniformLocation>,
}

struct Target {
    width: usize,
    height: usize,
    /// Which body owns each pixel's depth, the counterpart of `Frame::owner`.
    tags: glow::Texture,
    depth: glow::Texture,
    /// Colour, tags and depth: what the model is drawn into.
    scene: glow::Framebuffer,
    /// Colour alone, so the axis pass can *sample* the depth and tag textures
    /// that the scene pass wrote. A texture cannot be read and written in one
    /// pass, and the axis rule has to read both.
    overlay: glow::Framebuffer,
}

struct Buffers {
    array: glow::VertexArray,
    vertices: glow::Buffer,
    /// The per-segment table of which bodies an axis may be seen through,
    /// uploaded as a texture because there are as many entries as there are
    /// bodies in the scene.
    seen: glow::Texture,
}

/// One vertex as the shaders want it: screen position in pixels, depth key,
/// straight-alpha colour, the body tag, and -- for an axis -- which of its
/// segments this is, so the shader can look up that segment's own rule.
#[repr(C)]
#[derive(Clone, Copy)]
struct GpuVertex {
    x: f32,
    y: f32,
    key: f32,
    colour: [u8; 4],
    tag: u32,
    segment: u32,
}

impl GpuVertex {
    fn new(v: Vertex, colour: Rgba, tag: u16, segment: u32) -> GpuVertex {
        GpuVertex { x: v.pos.x, y: v.pos.y, key: v.key, colour, tag: tag as u32, segment }
    }
}

const VERTEX_SOURCE: &str = r#"#version 330 core
layout(location = 0) in vec2 in_pos;
layout(location = 1) in float in_key;
layout(location = 2) in vec4 in_colour;
layout(location = 3) in uint in_tag;
layout(location = 4) in uint in_segment;

uniform vec2 u_viewport;
// Maps the rasterizer's depth key -- larger is nearer -- onto OpenGL's, where
// smaller is nearer, so `GL_LESS` decides exactly what `key <= stored` decides
// in the software renderer. `x` is the offset and `y` the scale: the nearest
// key in the frame lands just inside the front of the buffer and the furthest
// just inside the back, with room at each end for the depth bias a line gets.
uniform vec2 u_depth;

out vec4 v_colour;
flat out uint v_tag;
flat out uint v_segment;
out float v_depth;

void main() {
    // The rasterizer counts rows downwards from the top; OpenGL counts them
    // upwards from the bottom -- and egui, painting this texture, takes its
    // first row to be the top of the rect. The two flips cancel, so row 0 goes
    // to the *bottom* of the framebuffer here and comes out at the top on
    // screen. Getting this the other way round mirrors the whole viewport,
    // which reads as a plausible picture until it is measured against the
    // software renderer's.
    vec2 ndc = vec2(
        (in_pos.x / u_viewport.x) * 2.0 - 1.0,
        (in_pos.y / u_viewport.y) * 2.0 - 1.0
    );
    float depth = clamp(u_depth.x - in_key * u_depth.y, -1.0, 1.0);
    gl_Position = vec4(ndc, depth, 1.0);
    v_colour = in_colour;
    v_tag = in_tag;
    v_segment = in_segment;
    v_depth = depth * 0.5 + 0.5;
}
"#;

const SOLID_SOURCE: &str = r#"#version 330 core
in vec4 v_colour;
flat in uint v_tag;

layout(location = 0) out vec4 out_colour;
layout(location = 1) out uint out_tag;

void main() {
    out_colour = v_colour;
    out_tag = v_tag;
}
"#;

/// The origin axes, and the one rule that is not just a depth test.
///
/// A piece of an axis that loses the depth test is still drawn when the body
/// that won that pixel is one this piece is arriving at -- `seen`, indexed by
/// body tag and by which segment of the line this is. That is the whole of
/// `Frame::put_with`'s `through` argument, moved into a shader.
const AXIS_SOURCE: &str = r#"#version 330 core
in vec4 v_colour;
flat in uint v_segment;
in float v_depth;

uniform sampler2D u_depth_tex;
uniform usampler2D u_tag_tex;
uniform sampler2D u_seen;
uniform vec2 u_seen_size;

out vec4 out_colour;

void main() {
    ivec2 at = ivec2(gl_FragCoord.xy);
    float scene = texelFetch(u_depth_tex, at, 0).r;
    if (v_depth >= scene) {
        uint owner = texelFetch(u_tag_tex, at, 0).r;
        if (float(owner) >= u_seen_size.x) discard;
        float allowed = texelFetch(u_seen, ivec2(int(owner), int(v_segment)), 0).r;
        if (allowed < 0.5) discard;
    }
    out_colour = v_colour;
}
"#;

/// A full-screen gradient, the same one `Palette::background_at` lays down.
const BACKGROUND_VERTEX: &str = r#"#version 330 core
out vec2 v_uv;
void main() {
    // A triangle covering the viewport, from the vertex index alone.
    vec2 p = vec2((gl_VertexID << 1) & 2, gl_VertexID & 2);
    v_uv = p;
    gl_Position = vec4(p * 2.0 - 1.0, 1.0, 1.0);
}
"#;

const BACKGROUND_FRAGMENT: &str = r#"#version 330 core
in vec2 v_uv;
uniform vec4 u_top;
uniform vec4 u_bottom;
layout(location = 0) out vec4 out_colour;
layout(location = 1) out uint out_tag;
void main() {
    // Row 0 of the frame -- the top, and the palette's first colour -- is at
    // the bottom of the framebuffer, so `v_uv.y` of 0 is the palette's top.
    out_colour = mix(u_top, u_bottom, v_uv.y);
    out_tag = 0u;
}
"#;

impl Gpu {
    /// Compile the shaders and make the buffers. Fails, rather than panics, on
    /// a driver that will not have them -- the caller falls back to the CPU.
    pub fn new(gl: Arc<glow::Context>) -> Result<Gpu, String> {
        unsafe {
            let solid = Program::new(&gl, VERTEX_SOURCE, SOLID_SOURCE)?;
            let axis = Program::new(&gl, VERTEX_SOURCE, AXIS_SOURCE)?;
            let background = Program::new(&gl, BACKGROUND_VERTEX, BACKGROUND_FRAGMENT)?;
            let buffer = Buffers::new(&gl)?;
            Ok(Gpu { gl, solid, axis, background, target: None, colour: None, buffer, texture_id: None })
        }
    }
}

impl Program {
    unsafe fn new(gl: &glow::Context, vertex: &str, fragment: &str) -> Result<Program, String> {
        let program = gl.create_program()?;
        let mut shaders = Vec::new();
        for (kind, source) in [(glow::VERTEX_SHADER, vertex), (glow::FRAGMENT_SHADER, fragment)] {
            let shader = gl.create_shader(kind)?;
            gl.shader_source(shader, source);
            gl.compile_shader(shader);
            if !gl.get_shader_compile_status(shader) {
                let log = gl.get_shader_info_log(shader);
                gl.delete_shader(shader);
                for shader in shaders {
                    gl.delete_shader(shader);
                }
                gl.delete_program(program);
                return Err(format!("shader would not compile: {log}"));
            }
            gl.attach_shader(program, shader);
            shaders.push(shader);
        }
        gl.link_program(program);
        for shader in shaders {
            gl.detach_shader(program, shader);
            gl.delete_shader(shader);
        }
        if !gl.get_program_link_status(program) {
            let log = gl.get_program_info_log(program);
            gl.delete_program(program);
            return Err(format!("shaders would not link: {log}"));
        }
        let mut uniforms = std::collections::HashMap::new();
        let count = gl.get_active_uniforms(program);
        for index in 0..count {
            if let Some(uniform) = gl.get_active_uniform(program, index) {
                if let Some(location) = gl.get_uniform_location(program, &uniform.name) {
                    uniforms.insert(uniform.name, location);
                }
            }
        }
        Ok(Program { program, uniforms })
    }

    fn at(&self, name: &str) -> Option<&glow::UniformLocation> {
        self.uniforms.get(name)
    }
}

impl Buffers {
    unsafe fn new(gl: &glow::Context) -> Result<Buffers, String> {
        let array = gl.create_vertex_array()?;
        let vertices = gl.create_buffer()?;
        gl.bind_vertex_array(Some(array));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertices));
        let stride = std::mem::size_of::<GpuVertex>() as i32;
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 1, glow::FLOAT, false, stride, 8);
        // Normalised, so the shader sees the same 0..1 the palette's bytes mean.
        gl.enable_vertex_attrib_array(2);
        gl.vertex_attrib_pointer_f32(2, 4, glow::UNSIGNED_BYTE, true, stride, 12);
        gl.enable_vertex_attrib_array(3);
        gl.vertex_attrib_pointer_i32(3, 1, glow::UNSIGNED_INT, stride, 16);
        gl.enable_vertex_attrib_array(4);
        gl.vertex_attrib_pointer_i32(4, 1, glow::UNSIGNED_INT, stride, 20);
        gl.bind_vertex_array(None);

        let seen = gl.create_texture()?;
        gl.bind_texture(glow::TEXTURE_2D, Some(seen));
        for (name, value) in [
            (glow::TEXTURE_MIN_FILTER, glow::NEAREST),
            (glow::TEXTURE_MAG_FILTER, glow::NEAREST),
            (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
            (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
        ] {
            gl.tex_parameter_i32(glow::TEXTURE_2D, name, value as i32);
        }
        gl.bind_texture(glow::TEXTURE_2D, None);
        Ok(Buffers { array, vertices, seen })
    }
}

impl Target {
    unsafe fn new(gl: &glow::Context, colour: glow::Texture, width: usize, height: usize) -> Result<Target, String> {
        let plain = |texture: glow::Texture| {
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            for (name, value) in [
                (glow::TEXTURE_MIN_FILTER, glow::NEAREST),
                (glow::TEXTURE_MAG_FILTER, glow::LINEAR),
                (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
                (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
            ] {
                gl.tex_parameter_i32(glow::TEXTURE_2D, name, value as i32);
            }
        };
        let (w, h) = (width as i32, height as i32);

        // The tag buffer is exactly `Frame::owner`: one body number per pixel,
        // written only by the passes that write depth.
        let tags = gl.create_texture()?;
        plain(tags);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::R16UI as i32,
            w,
            h,
            0,
            glow::RED_INTEGER,
            glow::UNSIGNED_SHORT,
            glow::PixelUnpackData::Slice(None),
        );

        let depth = gl.create_texture()?;
        plain(depth);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::DEPTH_COMPONENT24 as i32,
            w,
            h,
            0,
            glow::DEPTH_COMPONENT,
            glow::UNSIGNED_INT,
            glow::PixelUnpackData::Slice(None),
        );
        gl.bind_texture(glow::TEXTURE_2D, None);

        let scene = gl.create_framebuffer()?;
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(scene));
        gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(colour), 0);
        gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT1, glow::TEXTURE_2D, Some(tags), 0);
        gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::DEPTH_ATTACHMENT, glow::TEXTURE_2D, Some(depth), 0);
        gl.draw_buffers(&[glow::COLOR_ATTACHMENT0, glow::COLOR_ATTACHMENT1]);
        let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
        if status != glow::FRAMEBUFFER_COMPLETE {
            return Err(format!("the offscreen target is not usable (status {status:#x})"));
        }

        // Colour only: the axis pass samples the depth and tag textures, and a
        // texture attached to the framebuffer being drawn into cannot be read.
        let overlay = gl.create_framebuffer()?;
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(overlay));
        gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(colour), 0);
        gl.draw_buffers(&[glow::COLOR_ATTACHMENT0]);
        let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
        if status != glow::FRAMEBUFFER_COMPLETE {
            return Err(format!("the overlay target is not usable (status {status:#x})"));
        }
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        Ok(Target { width, height, tags, depth, scene, overlay })
    }

    unsafe fn destroy(&self, gl: &glow::Context) {
        gl.delete_framebuffer(self.scene);
        gl.delete_framebuffer(self.overlay);
        gl.delete_texture(self.tags);
        gl.delete_texture(self.depth);
        // The colour texture belongs to egui once it has been registered, so
        // it is deliberately not deleted here -- see `render`, which keeps one
        // texture for the life of the application and reallocates its storage.
    }
}

/// The primitives sorted into the passes that draw them, in drawing order.
///
/// A GPU draws in batches, and the batches have to keep the order the software
/// renderer draws in wherever depth does not already decide it: the ground
/// grid goes under the model, the model writes depth and its own body tags,
/// and the ghosts blend over what is there without claiming any of it.
#[derive(Default)]
struct Passes {
    /// Grid lines: blended, depth-tested, and leaving no depth of their own.
    grid: Vec<GpuVertex>,
    /// Shaded faces: opaque, writing depth and tag.
    solids: Vec<GpuVertex>,
    /// Feature edges, selection outlines, plane marks: opaque lines, biased
    /// towards the eye, writing depth and tag.
    lines: Vec<GpuVertex>,
    /// A tool's preview: over the model, blended, depth-tested, and claiming
    /// neither depth nor tag. The same state as the grid and the opposite side
    /// of the model from it, which is the whole reason it is a pass of its own.
    overlay: Vec<GpuVertex>,
    /// Ghosts: blended, depth-tested, writing neither depth nor tag.
    ghosts: Vec<GpuVertex>,
    /// The glow of a body inside another one: blended over everything, with the
    /// depth test off in both directions.
    glow: Vec<GpuVertex>,
    /// The origin axes, with their own rule.
    axes: Vec<GpuVertex>,
    /// The smallest and largest depth key in the frame, so the whole scene can
    /// be mapped into the depth buffer's range.
    key_range: Option<(f32, f32)>,
}

impl Passes {
    fn saw(&mut self, key: f32) {
        self.key_range = Some(match self.key_range {
            None => (key, key),
            Some((lo, hi)) => (lo.min(key), hi.max(key)),
        });
    }

    fn triangle(&mut self, v: [Vertex; 3], colour: Rgba, tag: u16, write_depth: bool) {
        let into = if write_depth { &mut self.solids } else { &mut self.ghosts };
        for vertex in v {
            into.push(GpuVertex::new(vertex, colour, tag, 0));
        }
        for vertex in v {
            self.saw(vertex.key);
        }
    }

    fn line(&mut self, a: Vertex, b: Vertex, colour: Rgba, bias: f32, tag: u16, write_depth: bool) {
        // The bias is part of the depth the line is tested at, exactly as it is
        // in `Frame::line_inner`, so it is folded into the key here rather than
        // being some separate polygon offset the driver decides the size of.
        let (a, b) = (biased(a, bias), biased(b, bias));
        let into = if write_depth { &mut self.lines } else { &mut self.grid };
        into.push(GpuVertex::new(a, colour, tag, 0));
        into.push(GpuVertex::new(b, colour, tag, 0));
        self.saw(a.key);
        self.saw(b.key);
    }

    fn glow(&mut self, v: [Vertex; 3], colour: Rgba) {
        for vertex in v {
            self.glow.push(GpuVertex::new(vertex, colour, 0, 0));
            self.saw(vertex.key);
        }
    }

    fn overlay(&mut self, a: Vertex, b: Vertex, colour: Rgba, bias: f32) {
        let (a, b) = (biased(a, bias), biased(b, bias));
        self.overlay.push(GpuVertex::new(a, colour, 0, 0));
        self.overlay.push(GpuVertex::new(b, colour, 0, 0));
        self.saw(a.key);
        self.saw(b.key);
    }
}

fn biased(v: Vertex, bias: f32) -> Vertex {
    Vertex { pos: v.pos, key: v.key + bias }
}

/// The axis bias, matching `render.rs`'s own. Kept in step by the test that
/// draws the same scene through both engines.
const AXIS_BIAS: f32 = -5.0e-4;

impl Gpu {
    /// Draw `prepared` into the offscreen texture, and return the id egui can
    /// paint it with.
    pub fn render(&mut self, request: &Request<'_>, prepared: &Prepared) -> Result<egui::TextureId, String> {
        let [width, height] = request.size;
        let (width, height) = (width.max(1), height.max(1));
        let mut passes = Passes::default();
        for step in &prepared.steps {
            match *step {
                Step::Triangle { v, colour, tag, write_depth } => passes.triangle(v, colour, tag, write_depth),
                Step::Line { a, b, colour, bias, tag, write_depth } => {
                    passes.line(a, b, colour, bias, tag, write_depth)
                }
                Step::Overlay { a, b, colour, bias } => passes.overlay(a, b, colour, bias),
                Step::Glow { v, colour } => passes.glow(v, colour),
            }
        }
        // The axes carry which segment they are, so the shader can find that
        // segment's row in the `seen` table.
        let mut tags = 1usize;
        for (segment, step) in prepared.axes.iter().enumerate() {
            let (a, b) = (biased(step.a, AXIS_BIAS), biased(step.b, AXIS_BIAS));
            passes.axes.push(GpuVertex::new(a, step.colour, 0, segment as u32));
            passes.axes.push(GpuVertex::new(b, step.colour, 0, segment as u32));
            passes.saw(a.key);
            passes.saw(b.key);
            tags = tags.max(step.seen.len());
        }

        unsafe { self.draw(request, &passes, &prepared.axes, tags, width, height) }
    }

    unsafe fn draw(
        &mut self,
        request: &Request<'_>,
        passes: &Passes,
        axes: &[AxisStep],
        tags: usize,
        width: usize,
        height: usize,
    ) -> Result<egui::TextureId, String> {
        let gl = self.gl.clone();
        self.resize(&gl, width, height)?;
        let target = self.target.as_ref().expect("resize leaves a target");

        // Larger keys are nearer; OpenGL wants smaller nearer. `depth.x` is the
        // key that maps to the front of the buffer and `depth.y` the scale, so
        // the whole scene lands inside the range with a margin either side for
        // the biases.
        // The nearest key maps to `-1 + margin` and the furthest to
        // `1 - margin`, leaving room at each end for a line's own bias to move
        // it without falling out of the buffer -- and, at the far end, room to
        // stay in front of the cleared value, or the ground grid would lose the
        // depth test against nothing at all.
        let (lo, hi) = passes.key_range.unwrap_or((0.0, 1.0));
        let span = (hi - lo).max(1e-6);
        let scale = (2.0 - 2.0 * DEPTH_MARGIN) / span;
        let offset = (DEPTH_MARGIN - 1.0) + scale * hi;

        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(target.scene));
        gl.viewport(0, 0, width as i32, height as i32);
        gl.disable(glow::SCISSOR_TEST);
        gl.disable(glow::CULL_FACE);
        gl.depth_mask(true);
        gl.clear_depth_f64(1.0);
        gl.clear(glow::DEPTH_BUFFER_BIT);

        // The background, and with it the tag buffer cleared to "no body".
        gl.disable(glow::DEPTH_TEST);
        gl.disable(glow::BLEND);
        gl.use_program(Some(self.background.program));
        let top = request.palette.background_at(0, height);
        let bottom = request.palette.background_at(height.saturating_sub(1), height);
        if let Some(at) = self.background.at("u_top") {
            gl.uniform_4_f32_slice(Some(at), &as_float(top));
        }
        if let Some(at) = self.background.at("u_bottom") {
            gl.uniform_4_f32_slice(Some(at), &as_float(bottom));
        }
        gl.bind_vertex_array(Some(self.buffer.array));
        gl.draw_arrays(glow::TRIANGLES, 0, 3);

        gl.enable(glow::DEPTH_TEST);
        gl.depth_func(glow::LESS);
        gl.use_program(Some(self.solid.program));
        if let Some(at) = self.solid.at("u_viewport") {
            gl.uniform_2_f32(Some(at), width as f32, height as f32);
        }
        if let Some(at) = self.solid.at("u_depth") {
            gl.uniform_2_f32(Some(at), offset, scale);
        }

        // The grid: under the model, blended, and never claiming a pixel's
        // depth or its body -- exactly `write_depth: false` in the rasterizer.
        gl.enable(glow::BLEND);
        gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
        gl.depth_mask(false);
        gl.draw_buffers(&[glow::COLOR_ATTACHMENT0, glow::NONE]);
        self.batch(&gl, glow::LINES, &passes.grid);

        // The model: opaque, writing depth and the body tag an axis will ask
        // about.
        gl.disable(glow::BLEND);
        gl.depth_mask(true);
        gl.draw_buffers(&[glow::COLOR_ATTACHMENT0, glow::COLOR_ATTACHMENT1]);
        self.batch(&gl, glow::TRIANGLES, &passes.solids);
        self.batch(&gl, glow::LINES, &passes.lines);

        // Ghosts, and a tool's preview: blended over what is there, tested
        // against the model and claiming nothing. The preview is here rather
        // than with the grid because it is drawn *on* the model -- the grid
        // goes under it.
        gl.enable(glow::BLEND);
        gl.depth_mask(false);
        gl.draw_buffers(&[glow::COLOR_ATTACHMENT0, glow::NONE]);
        self.batch(&gl, glow::TRIANGLES, &passes.ghosts);
        self.batch(&gl, glow::LINES, &passes.overlay);

        // The glow of a body inside another one, over everything and tested
        // against nothing: what is in front of it is exactly what it has to be
        // seen through.
        if !passes.glow.is_empty() {
            gl.disable(glow::DEPTH_TEST);
            self.batch(&gl, glow::TRIANGLES, &passes.glow);
            gl.enable(glow::DEPTH_TEST);
        }

        // The axes, in the overlay pass, where the depth and tag buffers are
        // readable rather than attached.
        if !passes.axes.is_empty() {
            self.upload_seen(&gl, axes, tags);
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(target.overlay));
            gl.viewport(0, 0, width as i32, height as i32);
            gl.disable(glow::DEPTH_TEST);
            gl.depth_mask(false);
            gl.enable(glow::BLEND);
            gl.use_program(Some(self.axis.program));
            if let Some(at) = self.axis.at("u_viewport") {
                gl.uniform_2_f32(Some(at), width as f32, height as f32);
            }
            if let Some(at) = self.axis.at("u_depth") {
                gl.uniform_2_f32(Some(at), offset, scale);
            }
            if let Some(at) = self.axis.at("u_seen_size") {
                gl.uniform_2_f32(Some(at), tags as f32, axes.len().max(1) as f32);
            }
            let target = self.target.as_ref().expect("resize leaves a target");
            for (unit, (name, texture)) in
                [("u_depth_tex", target.depth), ("u_tag_tex", target.tags), ("u_seen", self.buffer.seen)]
                    .into_iter()
                    .enumerate()
            {
                gl.active_texture(glow::TEXTURE0 + unit as u32);
                gl.bind_texture(glow::TEXTURE_2D, Some(texture));
                if let Some(at) = self.axis.at(name) {
                    gl.uniform_1_i32(Some(at), unit as i32);
                }
            }
            self.batch(&gl, glow::LINES, &passes.axes);
            gl.active_texture(glow::TEXTURE0);
        }

        // Put the pipeline back the way egui expects to find it.
        gl.bind_vertex_array(None);
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        gl.disable(glow::DEPTH_TEST);
        gl.depth_mask(true);
        gl.use_program(None);

        self.texture_id.ok_or_else(|| "the frame was drawn but never registered with egui".to_string())
    }

    unsafe fn batch(&self, gl: &glow::Context, mode: u32, vertices: &[GpuVertex]) {
        if vertices.is_empty() {
            return;
        }
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.buffer.vertices));
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, as_bytes(vertices), glow::STREAM_DRAW);
        gl.draw_arrays(mode, 0, vertices.len() as i32);
    }

    /// The table the axis shader reads: one row per segment of an axis, one
    /// column per body tag, holding whether that segment may be seen through
    /// that body.
    unsafe fn upload_seen(&self, gl: &glow::Context, axes: &[AxisStep], tags: usize) {
        let mut table = vec![0u8; tags * axes.len().max(1)];
        for (segment, step) in axes.iter().enumerate() {
            for (tag, &allowed) in step.seen.iter().enumerate() {
                if allowed && tag < tags {
                    table[segment * tags + tag] = 255;
                }
            }
        }
        gl.bind_texture(glow::TEXTURE_2D, Some(self.buffer.seen));
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::R8 as i32,
            tags as i32,
            axes.len().max(1) as i32,
            0,
            glow::RED,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(Some(&table)),
        );
    }

    /// Make or remake the offscreen target. The colour texture keeps its
    /// identity across a resize so egui's texture id stays good.
    unsafe fn resize(&mut self, gl: &glow::Context, width: usize, height: usize) -> Result<(), String> {
        if let Some(target) = &self.target {
            if target.width == width && target.height == height {
                return Ok(());
            }
        }
        let colour = match self.colour {
            Some(colour) => colour,
            None => {
                let colour = gl.create_texture()?;
                self.colour = Some(colour);
                colour
            }
        };
        gl.bind_texture(glow::TEXTURE_2D, Some(colour));
        for (name, value) in [
            (glow::TEXTURE_MIN_FILTER, glow::LINEAR),
            (glow::TEXTURE_MAG_FILTER, glow::LINEAR),
            (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
            (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
        ] {
            gl.tex_parameter_i32(glow::TEXTURE_2D, name, value as i32);
        }
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA8 as i32,
            width as i32,
            height as i32,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(None),
        );
        if let Some(old) = self.target.take() {
            old.destroy(gl);
        }
        self.target = Some(Target::new(gl, colour, width, height)?);
        Ok(())
    }

    /// Make the colour texture, so egui has something to be given. The first
    /// real frame reallocates it to the viewport's size; the texture object --
    /// and so egui's id for it -- stays the same.
    pub fn prepare_texture(&mut self) -> Result<glow::Texture, String> {
        let gl = self.gl.clone();
        unsafe { self.resize(&gl, 1, 1)? };
        self.colour.ok_or_else(|| "the colour texture was not made".to_string())
    }

    pub fn set_texture_id(&mut self, id: egui::TextureId) {
        self.texture_id = Some(id);
    }
}

fn as_float(colour: Rgba) -> [f32; 4] {
    [colour[0] as f32 / 255.0, colour[1] as f32 / 255.0, colour[2] as f32 / 255.0, colour[3] as f32 / 255.0]
}

fn as_bytes(vertices: &[GpuVertex]) -> &[u8] {
    // `GpuVertex` is `repr(C)` and holds only numbers, so its bytes are what
    // OpenGL is being handed.
    unsafe { std::slice::from_raw_parts(vertices.as_ptr() as *const u8, std::mem::size_of_val(vertices)) }
}
