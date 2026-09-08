//! Turning the renderable steps into draw calls.

use super::*;
use crate::render::{AxisStep, Request};
use eframe::glow::{self, HasContext};

impl Gpu {
    pub(super) unsafe fn draw(
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
}
