//! One frame's passes, in drawing order.

use super::*;
use crate::render::Request;
use eframe::glow::{self, HasContext};

impl Gpu {
    #[allow(clippy::too_many_arguments)]
    pub(super) unsafe fn draw(
        &mut self,
        request: &Request<'_>,
        passes: &Passes,
        plan: &resident::Plan,
        ground: &ground::Ground,
        width: usize,
        height: usize,
    ) -> Result<egui::TextureId, String> {
        let gl = self.gl.clone();
        self.resize(&gl, width, height)?;
        let target = self.target.as_ref().expect("resize leaves a target");

        // Map keys (larger nearer) to OpenGL depth (smaller nearer): nearest to `-1 + margin`, furthest to
        // `1 - margin`, leaving room for line bias and keeping the grid in front of the cleared value.
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

        // The background, and the tag buffer cleared to "no body".
        gl.disable(glow::DEPTH_TEST);
        gl.disable(glow::BLEND);
        gl.use_program(Some(self.background.program));
        let top = request.palette.background_at(0, height);
        let bottom = request.palette.background_at(height.saturating_sub(1), height);
        set4(&gl, &self.background, "u_top", &as_float(top));
        set4(&gl, &self.background, "u_bottom", &as_float(bottom));
        gl.bind_vertex_array(Some(self.buffer.array));
        gl.draw_arrays(glow::TRIANGLES, 0, 3);

        gl.enable(glow::DEPTH_TEST);
        gl.depth_func(glow::LESS);
        gl.use_program(Some(self.solid.program));
        set2(&gl, &self.solid, "u_viewport", [width as f32, height as f32]);
        set2(&gl, &self.solid, "u_depth", [offset, scale]);

        let (view, section) = (&request.view, &request.section[..]);
        let viewport = [width as f32, height as f32];
        let depth = [offset, scale];

        // The grid: under the model, blended, writing neither depth nor tag (`write_depth: false`).
        gl.enable(glow::BLEND);
        gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
        gl.depth_mask(false);
        gl.draw_buffers(&[glow::COLOR_ATTACHMENT0, glow::NONE]);
        self.draw_grid(&gl, ground, view, viewport, depth);

        // The model: opaque, writing depth and the body tag axes ask about.
        gl.disable(glow::BLEND);
        gl.depth_mask(true);
        gl.draw_buffers(&[glow::COLOR_ATTACHMENT0, glow::COLOR_ATTACHMENT1]);
        gl.cull_face(glow::BACK);
        gl.front_face(resident::front_face(&request.view));
        self.draw_faces(&gl, &plan.solids, view, section, viewport, depth);
        self.draw_caps(&gl, &plan.caps, view, viewport, depth);
        if let Some(csg) = &request.live.csg {
            self.draw_csg(&gl, request, csg, viewport, depth);
        }
        // Faces are drawn and no lines yet: what `depth.rs` reads as the picture.
        self.copy_depth(&gl, width, height, depth);
        self.draw_lines(&gl, &plan.lines, view, section, viewport, depth);
        if let (Some(csg), Some(colour)) = (&request.live.csg, plan.csg_edges) {
            self.draw_csg_edges(&gl, csg, colour);
        }
        self.draw_outlines(&gl, &plan.outlines, view, section, viewport, depth);
        self.draw_crossings(&gl, &plan.crossings, view, viewport, depth);

        // Ghosts and tool previews: blended, depth-tested, writing nothing. The preview is drawn on the
        // model, unlike the grid under it.
        gl.enable(glow::BLEND);
        gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
        gl.depth_mask(false);
        gl.draw_buffers(&[glow::COLOR_ATTACHMENT0, glow::NONE]);
        self.draw_faces(&gl, &plan.ghosts, view, section, viewport, depth);
        self.draw_lines(&gl, &plan.overlays, view, section, viewport, depth);
        // Tool templates (issue 70) like ghosts, pulled towards the eye so a face lying on a body's
        // face does not fight it, and their edges where seen. Nothing of them is drawn through a body:
        // that made the body look removed.
        gl.enable(glow::POLYGON_OFFSET_FILL);
        gl.polygon_offset(-1.0, -4.0);
        self.draw_faces(&gl, &plan.templates, view, section, viewport, depth);
        gl.disable(glow::POLYGON_OFFSET_FILL);
        self.draw_lines(&gl, &plan.template_edges, view, section, viewport, depth);

        // Glows of buried bodies, over everything with no depth test.
        if !plan.glows.is_empty() {
            gl.disable(glow::DEPTH_TEST);
            self.draw_faces(&gl, &plan.glows, view, section, viewport, depth);
            gl.enable(glow::DEPTH_TEST);
        }

        // The axes, in the overlay pass where depth and tag buffers are readable.
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(target.overlay));
        gl.viewport(0, 0, width as i32, height as i32);
        gl.disable(glow::DEPTH_TEST);
        gl.depth_mask(false);
        gl.enable(glow::BLEND);
        self.draw_axes(&gl, ground, view, viewport, depth);

        // Restore the pipeline state egui expects.
        gl.bind_vertex_array(None);
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        gl.front_face(glow::CCW);
        gl.disable(glow::DEPTH_TEST);
        gl.depth_mask(true);
        gl.use_program(None);

        self.texture_id.ok_or_else(|| "the frame was drawn but never registered with egui".to_string())
    }
}
