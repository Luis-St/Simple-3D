//! Drawing one frame on the GPU.

use super::*;
use crate::render::Request;
use eframe::glow::{self, HasContext};
use std::sync::Arc;

impl Gpu {
    /// Compile the shaders and make the buffers. Fails, rather than panics, on
    /// a driver that will not have them -- the caller falls back to the CPU.
    pub fn new(gl: Arc<glow::Context>) -> Result<Gpu, String> {
        unsafe {
            let solid = Program::new(&gl, VERTEX_SOURCE, SOLID_SOURCE)?;
            let axis = Program::new(&gl, &axis_vertex(), AXIS_FRAGMENT)?;
            let grid = Program::new(&gl, &grid_vertex(), GRID_FRAGMENT)?;
            let background = Program::new(&gl, BACKGROUND_VERTEX, BACKGROUND_FRAGMENT)?;
            let faces = Program::new(&gl, &face_vertex(), FACE_FRAGMENT)?;
            let lines = Program::with_geometry(&gl, &line_vertex(), Some(&line_geometry()), SOLID_SOURCE)?;
            let outline = Program::with_geometry(&gl, OUTLINE_VERTEX, Some(&outline_geometry()), SOLID_SOURCE)?;
            let crossing = Program::with_geometry(&gl, &crossing_vertex(), Some(&crossing_geometry()), SOLID_SOURCE)?;
            let csg_peel = Program::new(&gl, &face_vertex(), CSG_PEEL_FRAGMENT)?;
            let csg_count = Program::new(&gl, &face_vertex(), CSG_COUNT_FRAGMENT)?;
            let csg_resolve = Program::new(&gl, BACKGROUND_VERTEX, CSG_RESOLVE_FRAGMENT)?;
            // As wide as the driver allows, up to a size that keeps a table of
            // a few thousand entries from being mostly padding.
            let table_width = gl.get_parameter_i32(glow::MAX_TEXTURE_SIZE).clamp(1024, 8192) as usize;
            let buffer = Buffers::new(&gl)?;
            let ground = GroundBuffers::new(&gl)?;
            let depth = depth::DepthReadback::new(&gl)?;
            Ok(Gpu {
                gl,
                solid,
                axis,
                grid,
                background,
                faces,
                lines,
                outline,
                crossing,
                csg_peel,
                csg_count,
                csg_resolve,
                csg_targets: None,
                csg_half: None,
                table_width,
                resident: std::collections::HashMap::new(),
                preview: None,
                target: None,
                colour: None,
                buffer,
                ground,
                depth,
                texture_id: None,
            })
        }
    }
}

impl Gpu {
    /// Draw `request` into the offscreen texture, and return the id egui can
    /// paint it with.
    ///
    /// Nothing is prepared for it on the CPU: the meshes are on the card, and
    /// the grid, the axes and a tool's preview are drawn by shaders from the
    /// handful of numbers that decide them (`ground.rs`).
    pub fn render(&mut self, request: &Request<'_>) -> Result<egui::TextureId, String> {
        let [width, height] = request.size;
        let (width, height) = (width.max(1), height.max(1));
        let gl = self.gl.clone();
        self.refresh_preview(request);
        let mut plan = resident::plan(request);
        let preview = self.preview.take();
        let half = self.csg_half.take();
        let mut extra: Vec<&crate::render::Renderable> = preview.iter().map(|(_, lines)| lines).collect();
        if let Some(lines) = extra.first() {
            plan.overlays.push(resident::LineDraw::preview(lines.id, request.palette.selected));
        }
        // A boolean drawn per pixel: the shapes it is drawn from, and the
        // section's half-space as one more (`csg.rs`).
        let leaves = request.live.csg.iter().flat_map(|csg| csg.leaves.iter().map(|(leaf, _)| *leaf));
        extra.extend(leaves.chain(half.iter().map(|(_, shape)| shape)));
        plan.csg.extend(extra.iter().skip(preview.is_some() as usize).map(|shape| shape.id));
        let kept = unsafe { self.keep_resident(&gl, request, &plan, &extra) };
        self.preview = preview;
        self.csg_half = half;
        kept?;
        if request.live.csg.is_some() {
            self.refresh_half_space(request);
            unsafe {
                self.keep_half_space(&gl)?;
                self.ensure_csg_targets(&gl, width, height)?;
            }
        }
        let mut passes = Passes::default();
        self.see_resident(request, &plan, &mut passes);
        self.see_csg(request, &mut passes);
        self.place_caps(&request.view, &mut plan, &mut passes);
        let ground = ground::prepare(request, &mut passes);
        unsafe { self.draw(request, &passes, &plan, &ground, width, height) }
    }
}
