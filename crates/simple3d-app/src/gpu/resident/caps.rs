//! Filling a section's cut, in the colour of the body it opens.

use super::*;

impl Gpu {
    /// A section's caps. The stencil counts how often the kept surface winds round each pixel
    /// (front faces up, back faces down) in its low bits, winding rather than parity so
    /// overlapping bodies count as material, and the plane's box polygon marks where that is
    /// non-zero and in view with the high bit. There the nearest back face behind the plane is the
    /// inside of the body cut, so it is drawn in that body's colour, shaded as the plane (issue
    /// 114), and the plane's depth is written over it.
    ///
    /// Expects the model pass's state, and restores it.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::gpu) unsafe fn draw_caps(
        &self,
        gl: &glow::Context,
        draws: &[CapDraw],
        view: &View,
        viewport: [f32; 2],
        depth: [f32; 2],
    ) {
        const WINDING: u32 = 0x7F;
        const CAPPED: u32 = 0x80;
        let program = &self.faces;
        for cap in draws {
            let Some(resident) = self.resident.get(&cap.id) else { continue };
            let Some(faces) = &resident.faces else { continue };
            if cap.fill.is_empty() {
                continue;
            }
            gl.enable(glow::STENCIL_TEST);
            gl.stencil_mask(0xFF);
            gl.clear_stencil(0);
            gl.clear(glow::STENCIL_BUFFER_BIT);
            gl.color_mask(false, false, false, false);
            gl.depth_mask(false);
            gl.disable(glow::DEPTH_TEST);
            gl.stencil_mask(WINDING);
            gl.stencil_func(glow::ALWAYS, 0, 0xFF);
            gl.stencil_op_separate(glow::FRONT, glow::KEEP, glow::KEEP, glow::INCR_WRAP);
            gl.stencil_op_separate(glow::BACK, glow::KEEP, glow::KEEP, glow::DECR_WRAP);

            gl.use_program(Some(program.program));
            gl.enable(glow::CLIP_DISTANCE0);
            gl.enable(glow::CLIP_DISTANCE1);
            set2(gl, program, "u_viewport", viewport);
            set2(gl, program, "u_depth", depth);
            set_forward(gl, program, view);
            set_i32(gl, program, "u_table_width", self.table_width as i32);
            set_i32(gl, program, "u_paint", 0);
            // Counted against the cap's whole face only: the window box and other sections trim
            // the cap afterwards.
            set_projection(gl, program, resident, view, &[], &cap.placing);
            set4(gl, program, "u_clip", &resident.clip(Some(cap.plane), &cap.placing));
            set_i32(gl, program, "u_mode", GHOST);
            set_i32(gl, program, "u_painted", 0);
            gl.bind_vertex_array(Some(faces.array));
            gl.draw_elements(glow::TRIANGLES, faces.count, glow::UNSIGNED_INT, 0);

            // The cap's own pixels, where the model in front leaves it visible.
            gl.enable(glow::DEPTH_TEST);
            gl.stencil_mask(CAPPED);
            gl.stencil_func(glow::NOTEQUAL, CAPPED as i32, WINDING);
            gl.stencil_op(glow::KEEP, glow::KEEP, glow::REPLACE);
            gl.use_program(Some(self.solid.program));
            gl.bind_vertex_array(Some(self.buffer.array));
            self.batch(gl, glow::TRIANGLES, &cap.fill);

            // Their colour, from the back faces of the body cut.
            gl.color_mask(true, true, true, true);
            gl.depth_mask(true);
            gl.stencil_func(glow::EQUAL, CAPPED as i32, CAPPED);
            gl.stencil_op(glow::KEEP, glow::KEEP, glow::KEEP);
            gl.use_program(Some(program.program));
            set_i32(gl, program, "u_mode", CAP);
            set4(gl, program, "u_base", &cap.base.map(|c| c as f32));
            let normal = cap.plane.normal;
            if let Some(at) = program.at("u_cap_normal") {
                gl.uniform_3_f32(Some(at), normal.x as f32, normal.y as f32, normal.z as f32);
            }
            set_u32(gl, program, "u_tag_base", 0);
            set_i32(gl, program, "u_painted", resident.paint.is_some() as i32);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, resident.paint);
            gl.enable(glow::CULL_FACE);
            gl.cull_face(glow::FRONT);
            gl.bind_vertex_array(Some(faces.array));
            gl.draw_elements(glow::TRIANGLES, faces.count, glow::UNSIGNED_INT, 0);
            gl.cull_face(glow::BACK);
            gl.disable(glow::CULL_FACE);
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl.disable(glow::CLIP_DISTANCE0);
            gl.disable(glow::CLIP_DISTANCE1);

            // And the plane's depth, for what is drawn over the cap.
            gl.color_mask(false, false, false, false);
            gl.depth_func(glow::ALWAYS);
            gl.use_program(Some(self.solid.program));
            gl.bind_vertex_array(Some(self.buffer.array));
            self.batch(gl, glow::TRIANGLES, &cap.fill);
            gl.depth_func(glow::LESS);
            gl.color_mask(true, true, true, true);
            gl.stencil_mask(0xFF);
            gl.disable(glow::STENCIL_TEST);
        }
        gl.bind_vertex_array(Some(self.buffer.array));
    }
}
