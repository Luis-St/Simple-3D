//! Drawing one frame on the GPU.

use super::*;
use crate::render::{Prepared, Request, Step};
use eframe::glow::{self, HasContext};
use std::sync::Arc;

impl Gpu {
    /// Compile the shaders and make the buffers. Fails, rather than panics, on
    /// a driver that will not have them -- the caller falls back to the CPU.
    pub fn new(gl: Arc<glow::Context>) -> Result<Gpu, String> {
        unsafe {
            let solid = Program::new(&gl, VERTEX_SOURCE, SOLID_SOURCE)?;
            let axis = Program::new(&gl, VERTEX_SOURCE, AXIS_SOURCE)?;
            let background = Program::new(&gl, BACKGROUND_VERTEX, BACKGROUND_FRAGMENT)?;
            let faces = Program::new(&gl, &face_vertex(), FACE_FRAGMENT)?;
            let lines = Program::with_geometry(&gl, &line_vertex(), Some(&line_geometry()), SOLID_SOURCE)?;
            let outline = Program::with_geometry(&gl, OUTLINE_VERTEX, Some(&outline_geometry()), SOLID_SOURCE)?;
            let crossing = Program::with_geometry(&gl, &crossing_vertex(), Some(&crossing_geometry()), SOLID_SOURCE)?;
            // As wide as the driver allows, up to a size that keeps a table of
            // a few thousand entries from being mostly padding.
            let table_width = gl.get_parameter_i32(glow::MAX_TEXTURE_SIZE).clamp(1024, 8192) as usize;
            let buffer = Buffers::new(&gl)?;
            Ok(Gpu {
                gl,
                solid,
                axis,
                background,
                faces,
                lines,
                outline,
                crossing,
                table_width,
                resident: std::collections::HashMap::new(),
                target: None,
                colour: None,
                buffer,
                texture_id: None,
            })
        }
    }
}

impl Gpu {
    /// Draw `prepared` into the offscreen texture, and return the id egui can
    /// paint it with.
    ///
    /// `prepared` is expected to have been made with
    /// [`crate::render::Geometry::Resident`]: the items' own faces and lines are
    /// drawn from the copies of their meshes kept on the card, and a step list
    /// that carried them as well would draw them twice.
    pub fn render(&mut self, request: &Request<'_>, prepared: &Prepared) -> Result<egui::TextureId, String> {
        let [width, height] = request.size;
        let (width, height) = (width.max(1), height.max(1));
        let gl = self.gl.clone();
        let mut plan = resident::plan(request);
        unsafe { self.keep_resident(&gl, request, &plan)? };
        let mut passes = Passes::default();
        self.see_resident(request, &mut passes);
        self.place_caps(&request.view, &mut plan, &mut passes);
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

        unsafe { self.draw(request, &passes, &plan, &prepared.axes, tags, width, height) }
    }
}
