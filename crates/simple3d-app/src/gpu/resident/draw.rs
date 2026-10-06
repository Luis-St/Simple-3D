//! Drawing resident meshes in the frame's passes.

use super::*;

impl Gpu {
    /// Faces from the card, in whichever pass the caller has set up.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::gpu) unsafe fn draw_faces(
        &self,
        gl: &glow::Context,
        draws: &[FaceDraw],
        view: &View,
        section: &[Plane],
        viewport: [f32; 2],
        depth: [f32; 2],
    ) {
        if draws.is_empty() {
            return;
        }
        let program = &self.faces;
        gl.use_program(Some(program.program));
        gl.enable(glow::CLIP_DISTANCE0);
        gl.enable(glow::CLIP_DISTANCE1);
        set2(gl, program, "u_viewport", viewport);
        set2(gl, program, "u_depth", depth);
        set_forward(gl, program, view);
        set_i32(gl, program, "u_table_width", self.table_width as i32);
        set_i32(gl, program, "u_paint", 0);
        for draw in draws {
            let Some(resident) = self.resident.get(&draw.id) else { continue };
            let Some(faces) = &resident.faces else { continue };
            set_projection(gl, program, resident, view, section, &draw.placing);
            set4(gl, program, "u_base", &draw.base.map(|c| c as f32));
            set_i32(gl, program, "u_mode", draw.mode);
            set_u32(gl, program, "u_tag_base", draw.tag_base as u32);
            set_i32(gl, program, "u_painted", resident.paint.is_some() as i32);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, resident.paint);
            // Solids are back-face culled by the pipeline; ghosts and glows show the whole shell.
            if draw.mode == SOLID {
                gl.enable(glow::CULL_FACE);
            } else {
                gl.disable(glow::CULL_FACE);
            }
            gl.bind_vertex_array(Some(faces.array));
            gl.draw_elements(glow::TRIANGLES, faces.count, glow::UNSIGNED_INT, 0);
        }
        gl.disable(glow::CULL_FACE);
        gl.disable(glow::CLIP_DISTANCE0);
        gl.disable(glow::CLIP_DISTANCE1);
        gl.bind_texture(glow::TEXTURE_2D, None);
        gl.bind_vertex_array(Some(self.buffer.array));
    }

    /// Feature edges and the wireframe from the card.
    pub(in crate::gpu) unsafe fn draw_lines(
        &self,
        gl: &glow::Context,
        draws: &[LineDraw],
        view: &View,
        section: &[Plane],
        viewport: [f32; 2],
        depth: [f32; 2],
    ) {
        if draws.is_empty() {
            return;
        }
        let program = &self.lines;
        gl.use_program(Some(program.program));
        gl.enable(glow::CLIP_DISTANCE0);
        set2(gl, program, "u_viewport", viewport);
        set2(gl, program, "u_depth", depth);
        for draw in draws {
            let Some(resident) = self.resident.get(&draw.id) else { continue };
            let Some(edges) = &resident.edges else { continue };
            if edges.count == 0 {
                continue;
            }
            set_projection(gl, program, resident, view, section, &draw.placing);
            set4(gl, program, "u_colour", &as_float(draw.colour));
            set_f32(gl, program, "u_bias", draw.bias);
            // Only lines lying on the drawn faces; the wireframe has none to sink into.
            self.set_edge_slope(gl, program, resident, draw.bias > 0.0);
            set_i32(gl, program, "u_tagged", draw.tag_base.is_some() as i32);
            set_u32(gl, program, "u_tag_base", draw.tag_base.unwrap_or(0) as u32);
            gl.bind_vertex_array(Some(edges.array));
            gl.draw_elements(glow::LINES, edges.count, glow::UNSIGNED_INT, 0);
        }
        gl.disable(glow::CLIP_DISTANCE0);
        gl.active_texture(glow::TEXTURE1);
        gl.bind_texture(glow::TEXTURE_2D, None);
        gl.active_texture(glow::TEXTURE0);
        gl.bind_vertex_array(Some(self.buffer.array));
    }

    /// Feature edges' slope bias (`line_geometry`), from `resident`'s neighbouring faces on texture
    /// unit 1; `draw_lines` unbinds it again.
    pub(in crate::gpu) unsafe fn set_edge_slope(
        &self,
        gl: &glow::Context,
        program: &Program,
        resident: &Resident,
        wanted: bool,
    ) {
        let sloped = wanted && resident.edge_faces.is_some();
        set_i32(gl, program, "u_sloped", sloped as i32);
        if !sloped {
            return;
        }
        gl.active_texture(glow::TEXTURE1);
        gl.bind_texture(glow::TEXTURE_2D, resident.edge_faces);
        gl.active_texture(glow::TEXTURE0);
        set_i32(gl, program, "u_far_corners", 1);
        set_i32(gl, program, "u_table_width", self.table_width as i32);
        set_f32(gl, program, "u_slope_pixels", EDGE_SLOPE_PIXELS);
        set_f32(gl, program, "u_slope_cap", EDGE_SLOPE_CAP);
        set_f32(gl, program, "u_edge_on", EDGE_ON as f32);
    }

    /// Selected and glowing bodies' outlines.
    pub(in crate::gpu) unsafe fn draw_outlines(
        &self,
        gl: &glow::Context,
        draws: &[OutlineDraw],
        view: &View,
        section: &[Plane],
        viewport: [f32; 2],
        depth: [f32; 2],
    ) {
        if draws.is_empty() {
            return;
        }
        let program = &self.outline;
        gl.use_program(Some(program.program));
        gl.enable(glow::CLIP_DISTANCE0);
        set2(gl, program, "u_viewport", viewport);
        set2(gl, program, "u_depth", depth);
        set_forward(gl, program, view);
        set_i32(gl, program, "u_table_width", self.table_width as i32);
        set_f32(gl, program, "u_edge_on", EDGE_ON as f32);
        set_f32(gl, program, "u_crease", SELECTION_CREASE.to_radians().cos() as f32);
        set_f32(gl, program, "u_bias", SELECTION_BIAS);
        set_f32(gl, program, "u_slope_pixels", SELECTION_SLOPE_PIXELS);
        set_f32(gl, program, "u_slope_cap", SELECTION_SLOPE_CAP);
        for (unit, name) in ["u_positions", "u_triangles", "u_bodies"].into_iter().enumerate() {
            set_i32(gl, program, name, unit as i32);
        }
        for draw in draws {
            let Some(resident) = self.resident.get(&draw.id) else { continue };
            let (Some(outline), Some(tables)) = (&resident.outline, &resident.tables) else { continue };
            if outline.count == 0 {
                continue;
            }
            set_projection(gl, program, resident, view, section, &draw.placing);
            set4(gl, program, "u_colour", &as_float(draw.colour));
            set_u32(gl, program, "u_tag_base", draw.tag_base as u32);
            set_i32(gl, program, "u_all_creases", draw.all_creases as i32);
            for (unit, texture) in [tables.positions, tables.triangles, tables.bodies].into_iter().enumerate() {
                gl.active_texture(glow::TEXTURE0 + unit as u32);
                gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            }
            gl.bind_vertex_array(Some(outline.array));
            gl.draw_arrays(glow::POINTS, 0, outline.count);
        }
        for unit in 0..3 {
            gl.active_texture(glow::TEXTURE0 + unit);
            gl.bind_texture(glow::TEXTURE_2D, None);
        }
        gl.active_texture(glow::TEXTURE0);
        gl.disable(glow::CLIP_DISTANCE0);
        gl.bind_vertex_array(Some(self.buffer.array));
    }

    /// Plane marks and section cut edges on the resident meshes.
    pub(in crate::gpu) unsafe fn draw_crossings(
        &self,
        gl: &glow::Context,
        draws: &[CrossingDraw],
        view: &View,
        viewport: [f32; 2],
        depth: [f32; 2],
    ) {
        if draws.is_empty() {
            return;
        }
        let program = &self.crossing;
        gl.use_program(Some(program.program));
        gl.enable(glow::CLIP_DISTANCE0);
        set2(gl, program, "u_viewport", viewport);
        set2(gl, program, "u_depth", depth);
        set_f32(gl, program, "u_bias", MARK_BIAS);
        for draw in draws {
            let Some(resident) = self.resident.get(&draw.id) else { continue };
            let Some(faces) = &resident.faces else { continue };
            set_projection(gl, program, resident, view, &draw.cuts, &draw.placing);
            set_within(gl, program, resident, &draw.placing, &draw.within);
            let planes: Vec<[f32; 4]> =
                draw.planes.iter().map(|(plane, _)| resident.plane(plane, &draw.placing)).collect();
            let colours: Vec<[f32; 4]> = draw.planes.iter().map(|&(_, colour)| as_float(colour)).collect();
            set_i32(gl, program, "u_planes", planes.len() as i32);
            set4(gl, program, "u_plane[0]", planes.as_flattened());
            set4(gl, program, "u_plane_colour[0]", colours.as_flattened());
            // A few times the single-precision error of the largest number in play.
            let offset = planes.iter().fold(0.0_f64, |most, plane| most.max(plane[3].abs() as f64));
            set_f32(gl, program, "u_on_plane", ((resident.extent() + offset) * 4e-7 + 1e-9) as f32);
            gl.bind_vertex_array(Some(faces.array));
            gl.draw_elements(glow::TRIANGLES, faces.count, glow::UNSIGNED_INT, 0);
        }
        gl.disable(glow::CLIP_DISTANCE0);
        gl.bind_vertex_array(Some(self.buffer.array));
    }
}
