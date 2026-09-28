//! The boolean's layers, peeled, counted and resolved into the model pass.

use super::*;

impl Gpu {
    /// Draw the boolean into the model pass. Expects the model pass's framebuffer and state, and
    /// leaves them bound.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::gpu) unsafe fn draw_csg(
        &self,
        gl: &glow::Context,
        request: &Request<'_>,
        csg: &CsgPreview<'_>,
        viewport: [f32; 2],
        depth: [f32; 2],
    ) {
        let (Some(targets), Some(scene)) = (&self.csg_targets, &self.target) else { return };
        let view = &request.view;
        let mut leaves: Vec<(&Renderable, Option<simple3d_core::xform::Xform>)> = csg.leaves.clone();
        let mut program = csg.program.clone();
        // Sections come last: the whole intersected with each kept half-space, or with a windowed
        // cut's box subtracted.
        let half_leaf = self.csg_half.as_ref().filter(|(_, shapes)| !shapes.is_empty()).map(|_| leaves.len());
        for (shape, op) in self.csg_half.iter().flat_map(|(_, shapes)| shapes.iter()) {
            program.push(leaves.len() as i32);
            program.push(*op);
            leaves.push((shape, None));
        }
        let drawn: Vec<(&resident::Resident, Placing)> = leaves
            .iter()
            .filter_map(|(leaf, moved)| Some((self.resident.get(&leaf.id)?, Placing::moved(*moved))))
            .collect();
        if drawn.len() != leaves.len() || leaves.len() > MAX_SHAPES {
            return;
        }
        let (width, height) = (targets.width as i32, targets.height as i32);
        let marks = resident::mark_planes(request);
        let firsts: Vec<u32> = leaves.iter().map(|(leaf, _)| leaf.mesh.tag(0)).collect();
        let inherited = inherit::inherited(&program, &firsts, half_leaf);

        gl.disable(glow::CULL_FACE);
        gl.disable(glow::BLEND);
        gl.disable(glow::CLIP_DISTANCE0);
        gl.disable(glow::CLIP_DISTANCE1);
        // The stencil (and the `done` target, for passes that cannot see it) marks pixels a layer has
        // already drawn, which are not peeled or counted again.
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(scene.scene));
        gl.stencil_mask(0xFF);
        gl.clear_stencil(0);
        gl.clear(glow::STENCIL_BUFFER_BIT);
        gl.draw_buffers(&[glow::NONE, glow::NONE, glow::COLOR_ATTACHMENT2]);
        gl.clear_buffer_f32_slice(glow::COLOR, 2, &[0.0; 4]);

        let plan = targets.plan(gl);
        let layers = match plan {
            LayerPlan::Asking => MAX_LAYERS,
            LayerPlan::Fixed(layers) => layers,
        };
        let mut peeled = layers;
        for layer in 0..layers {
            let (now, before) = (layer % 2, (layer + 1) % 2);

            // The layer: the nearest face behind the one before.
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(targets.peel[now]));
            gl.viewport(0, 0, width, height);
            gl.draw_buffers(&[glow::COLOR_ATTACHMENT0, glow::COLOR_ATTACHMENT1]);
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LESS);
            gl.depth_mask(true);
            gl.clear_depth_f64(1.0);
            gl.clear(glow::DEPTH_BUFFER_BIT);
            let peel = &self.csg_peel;
            gl.use_program(Some(peel.program));
            set2(gl, peel, "u_viewport", viewport);
            set2(gl, peel, "u_depth", depth);
            set_forward(gl, peel, view);
            set_i32(gl, peel, "u_first", (layer == 0) as i32);
            set_i32(gl, peel, "u_table_width", self.table_width as i32);
            set_i32(gl, peel, "u_paint", 0);
            set_i32(gl, peel, "u_previous", 1);
            gl.active_texture(glow::TEXTURE1);
            gl.bind_texture(glow::TEXTURE_2D, Some(targets.depth[before]));
            set_i32(gl, peel, "u_done", 2);
            gl.active_texture(glow::TEXTURE2);
            gl.bind_texture(glow::TEXTURE_2D, Some(scene.done));
            set_i32(gl, peel, "u_scene_depth", 3);
            gl.active_texture(glow::TEXTURE3);
            gl.bind_texture(glow::TEXTURE_2D, Some(scene.depth));
            if let Some(at) = peel.at("u_base") {
                let solid = request.palette.solid;
                gl.uniform_4_f32_slice(Some(at), &[solid[0] as f32, solid[1] as f32, solid[2] as f32, 255.0]);
            }
            if let Some(at) = peel.at("u_plane_colour[0]") {
                let mut colours = [[0.0_f32; 4]; 3];
                for (colour, (_, rgba)) in colours.iter_mut().zip(&marks) {
                    *colour = as_float(*rgba);
                }
                gl.uniform_4_f32_slice(Some(at), colours.as_flattened());
            }
            gl.begin_query(glow::ANY_SAMPLES_PASSED, targets.queries[layer]);
            for (index, (resident, placing)) in drawn.iter().enumerate() {
                let Some(faces) = &resident.faces else { continue };
                set_projection(gl, peel, resident, view, &[], placing);
                // The section's half-space is the cut, not model surface, and cuts are not marked.
                let marked = if half_leaf.is_some_and(|first| index >= first) { 0 } else { marks.len() };
                set_i32(gl, peel, "u_planes", marked as i32);
                if let Some(at) = peel.at("u_plane[0]") {
                    let mut planes = [[0.0_f32; 4]; 3];
                    for (plane, (mark, _)) in planes.iter_mut().zip(&marks) {
                        *plane = resident.plane(mark, placing);
                    }
                    gl.uniform_4_f32_slice(Some(at), planes.as_flattened());
                }
                set_u32(gl, peel, "u_leaf", index as u32);
                set4(gl, peel, "u_inherit", &inherited[index].unwrap_or([0.0; 4]));
                set_i32(gl, peel, "u_painted", resident.paint.is_some() as i32);
                gl.active_texture(glow::TEXTURE0);
                gl.bind_texture(glow::TEXTURE_2D, resident.paint);
                gl.bind_vertex_array(Some(faces.array));
                gl.draw_elements(glow::TRIANGLES, faces.count, glow::UNSIGNED_INT, 0);
            }
            gl.end_query(glow::ANY_SAMPLES_PASSED);
            if matches!(plan, LayerPlan::Asking)
                && gl.get_query_parameter_u32(targets.queries[layer], glow::QUERY_RESULT) == 0
            {
                peeled = layer + 1;
                break;
            }

            // Which shapes the layer's point is inside, 32 at a time: counts per channel, then packed into bits.
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(targets.pack));
            gl.clear_buffer_u32_slice(glow::COLOR, 0, &[0; 4]);
            for (group, shapes) in drawn.chunks(32).enumerate() {
                let textures_used = shapes.len().div_ceil(4);
                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(targets.count[now]));
                let buffers: Vec<u32> =
                    (0..textures_used as u32).map(|index| glow::COLOR_ATTACHMENT0 + index).collect();
                gl.draw_buffers(&buffers);
                for index in 0..textures_used as u32 {
                    gl.clear_buffer_f32_slice(glow::COLOR, index, &[0.0; 4]);
                }
                gl.depth_mask(false);
                gl.enable(glow::BLEND);
                gl.blend_func(glow::ONE, glow::ONE);
                let count = &self.csg_count;
                gl.use_program(Some(count.program));
                set2(gl, count, "u_viewport", viewport);
                set2(gl, count, "u_depth", depth);
                for (index, (resident, placing)) in shapes.iter().enumerate() {
                    let Some(faces) = &resident.faces else { continue };
                    for texture in 0..textures_used {
                        let on = |channel: usize| texture == index / 4 && channel == index % 4;
                        gl.color_mask_draw_buffer(texture as u32, on(0), on(1), on(2), on(3));
                    }
                    set_projection(gl, count, resident, view, &[], placing);
                    gl.bind_vertex_array(Some(faces.array));
                    gl.draw_elements(glow::TRIANGLES, faces.count, glow::UNSIGNED_INT, 0);
                }
                for texture in 0..textures_used as u32 {
                    gl.color_mask_draw_buffer(texture, true, true, true, true);
                }
                gl.disable(glow::BLEND);
                gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);

                // The group's counts as bits, into its own channel.
                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(targets.pack));
                gl.disable(glow::DEPTH_TEST);
                let on = |channel: usize| channel == group;
                gl.color_mask(on(0), on(1), on(2), on(3));
                let pack = &self.csg_pack;
                gl.use_program(Some(pack.program));
                for (unit, texture) in targets.counts.iter().enumerate() {
                    gl.active_texture(glow::TEXTURE0 + unit as u32);
                    gl.bind_texture(glow::TEXTURE_2D, Some(*texture));
                    set_i32(gl, pack, &format!("u_count_{unit}"), unit as i32);
                }
                set_i32(gl, pack, "u_shapes", shapes.len() as i32);
                gl.bind_vertex_array(Some(self.buffer.array));
                gl.draw_arrays(glow::TRIANGLES, 0, 3);
                gl.color_mask(true, true, true, true);
                gl.enable(glow::DEPTH_TEST);
            }

            // The layer's points on the result's surface, into the frame.
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(scene.scene));
            gl.viewport(0, 0, width, height);
            gl.draw_buffers(&[glow::COLOR_ATTACHMENT0, glow::COLOR_ATTACHMENT1, glow::COLOR_ATTACHMENT2]);
            gl.depth_mask(true);
            // Drawn only where no layer has been; marked done where drawn and where the scene hides it.
            // The reference is what `REPLACE` writes, so "not done" is "not equal to 1".
            gl.enable(glow::STENCIL_TEST);
            gl.stencil_mask(1);
            gl.stencil_func(glow::NOTEQUAL, 1, 1);
            gl.stencil_op(glow::KEEP, glow::REPLACE, glow::REPLACE);
            let resolve = &self.csg_resolve;
            gl.use_program(Some(resolve.program));
            let textures = [
                ("u_layer", targets.depth[now]),
                ("u_leaf", targets.leaf),
                ("u_colour", targets.colour),
                ("u_inside", targets.inside),
            ];
            for (unit, (name, texture)) in textures.into_iter().enumerate() {
                gl.active_texture(glow::TEXTURE0 + unit as u32);
                gl.bind_texture(glow::TEXTURE_2D, Some(texture));
                set_i32(gl, resolve, name, unit as i32);
            }
            set_i32(gl, resolve, "u_length", program.len() as i32);
            if let Some(at) = resolve.at("u_program[0]") {
                gl.uniform_1_i32_slice(Some(at), &program);
            }
            set_u32(gl, resolve, "u_tag", csg.tag as u32);
            set_i32(gl, resolve, "u_half", half_leaf.map_or(-1, |first| first as i32));
            gl.bind_vertex_array(Some(self.buffer.array));
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            gl.disable(glow::STENCIL_TEST);
            for unit in 0..8 {
                gl.active_texture(glow::TEXTURE0 + unit);
                gl.bind_texture(glow::TEXTURE_2D, None);
            }
            gl.active_texture(glow::TEXTURE0);
        }

        targets.last.set(Some(peeled));

        // Restore the model pass's state.
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(scene.scene));
        gl.viewport(0, 0, width, height);
        gl.draw_buffers(&[glow::COLOR_ATTACHMENT0, glow::COLOR_ATTACHMENT1]);
        gl.enable(glow::DEPTH_TEST);
        gl.depth_func(glow::LESS);
        gl.depth_mask(true);
        gl.stencil_mask(0xFF);
        gl.bind_vertex_array(Some(self.buffer.array));
    }
}
