//! A boolean worked out per pixel, for the frames of a drag that changes it.
//!
//! Dragging a cutter, or a body inside a union it meets, changes the shape the
//! boolean makes, and working that shape out is the kernel's job -- seconds on
//! a large model, and far too long for a frame. What the frame needs is only
//! what the shape *looks* like, and that can be read off the shapes that go
//! into it without making it: a point of the picture is on the result's
//! surface where it is on one of the shapes' surfaces and the boolean says one
//! thing just in front of it and the other just behind (`App::live_csg`).
//!
//! The surfaces are taken a layer at a time from the front: each pass keeps,
//! at every pixel, the nearest face behind the layer before. For each layer the
//! faces of each shape in front of it are counted, one shape to a channel,
//! and an odd count says the layer's point is inside that shape. The
//! expression is then asked, per pixel, with the point's own shape counted as
//! both inside and outside; where the answers differ the point is drawn, at its
//! own depth, and the pixel is done. The first such layer is the surface the
//! eye sees, and the rest of the scene hides it or not by the depth test like
//! anything else.
//!
//! A section is one more shape: the kept half-space, as a box with a face on
//! the plane, intersected with the whole. Its face is the cap, drawn where the
//! cut runs through material, as `draw_caps` fills the scene's own.
//!
//! The shapes have to be closed for the counts to mean anything, and faces
//! of two shapes lying in one plane are taken one at a time, so either can
//! show where they meet: what the drag shows is a preview, and the evaluation
//! when the body is let go is the shape.

use super::*;
use crate::render::{CsgPreview, Renderable, Request};
use eframe::glow::{self, HasContext};
use resident::{set2, set_forward, set_i32, set_projection, Placing};
use simple3d_geom::section::Plane;
use simple3d_geom::{Mesh, Vec3};

/// The most layers a frame peels. A shape seldom has more than a handful of
/// surfaces along one ray; the peeling stops at the first layer with nothing
/// left in it, and this bounds a frame of a pathological one.
const MAX_LAYERS: usize = 32;

/// The most shapes a boolean is drawn from, the section's half-space among
/// them: a bit each of the 128 the resolve pass reads.
pub(crate) const MAX_SHAPES: usize = 128;

/// The fewest a frame that does not ask as it goes peels.
const MIN_LAYERS: usize = 4;

/// What the passes draw into, the size of the frame.
pub(crate) struct CsgTargets {
    width: usize,
    height: usize,
    /// Two layers' depths, one being peeled while the other is read.
    depth: [glow::Texture; 2],
    /// Which shape each pixel of the layer is on, and its colour there.
    leaf: glow::Texture,
    colour: glow::Texture,
    /// The counts, four shapes to a texture, for one group of up to 32 shapes
    /// at a time.
    counts: [glow::Texture; 8],
    /// Which shapes the layer's point is inside, a bit each: 32 to a channel,
    /// one channel per group, packed from the counts (`CSG_PACK_FRAGMENT`).
    inside: glow::Texture,
    peel: [glow::Framebuffer; 2],
    count: [glow::Framebuffer; 2],
    pack: glow::Framebuffer,
    /// One per layer: whether it had anything in it. See [`LayerPlan`].
    queries: Vec<glow::Query>,
    /// How many layers the last frame drew, and whether it waited on each to
    /// say so -- `None` when there was no last frame to go by.
    last: std::cell::Cell<Option<usize>>,
}

/// How many layers a frame peels, and whether it asks after each one.
///
/// Asking whether a layer had anything in it before drawing the next is a
/// wait for the card to finish it: up to 32 times a frame the processor sat
/// idle until the card caught up, and the card then sat idle while the next
/// layer was sent. So a frame instead draws as many layers as the last one
/// turned out to need, with two to spare, and reads what each held a frame
/// late, when the answer is long in. A layer beyond the last surface finds
/// nothing and draws nothing, so the spare ones cost only their passes; and a
/// frame whose every layer held something is followed by one with twice as
/// many. Only a frame with nothing to go by -- the first of a drag, or the
/// first after a resize -- still asks as it goes.
enum LayerPlan {
    Asking,
    Fixed(usize),
}

impl CsgTargets {
    unsafe fn new(gl: &glow::Context, width: usize, height: usize) -> Result<CsgTargets, String> {
        let texture = |internal: u32, format: u32, kind: u32| -> Result<glow::Texture, String> {
            let texture = gl.create_texture()?;
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            for (name, value) in [
                (glow::TEXTURE_MIN_FILTER, glow::NEAREST),
                (glow::TEXTURE_MAG_FILTER, glow::NEAREST),
                (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
                (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
            ] {
                gl.tex_parameter_i32(glow::TEXTURE_2D, name, value as i32);
            }
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
        let leaf = texture(glow::R8UI, glow::RED_INTEGER, glow::UNSIGNED_BYTE)?;
        let colour = texture(glow::RGBA8, glow::RGBA, glow::UNSIGNED_BYTE)?;
        let mut counts = Vec::with_capacity(8);
        for _ in 0..8 {
            counts.push(texture(glow::RGBA8, glow::RGBA, glow::UNSIGNED_BYTE)?);
        }
        let counts: [glow::Texture; 8] = counts.try_into().expect("eight made");
        let inside = texture(glow::RGBA32UI, glow::RGBA_INTEGER, glow::UNSIGNED_INT)?;
        gl.bind_texture(glow::TEXTURE_2D, None);

        let framebuffer = |depth: glow::Texture, colours: &[glow::Texture]| -> Result<glow::Framebuffer, String> {
            let framebuffer = gl.create_framebuffer()?;
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
            gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::DEPTH_ATTACHMENT, glow::TEXTURE_2D, Some(depth), 0);
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
        let peel = [framebuffer(depth[0], &[colour, leaf])?, framebuffer(depth[1], &[colour, leaf])?];
        let count = [framebuffer(depth[0], &counts)?, framebuffer(depth[1], &counts)?];
        // Only the one colour target: a full-frame pass with no depth to test.
        let pack = gl.create_framebuffer()?;
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(pack));
        gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(inside), 0);
        gl.draw_buffers(&[glow::COLOR_ATTACHMENT0]);
        if gl.check_framebuffer_status(glow::FRAMEBUFFER) != glow::FRAMEBUFFER_COMPLETE {
            return Err("the boolean preview's target is not usable".into());
        }
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
            peel,
            count,
            pack,
            queries,
            last: std::cell::Cell::new(None),
        })
    }

    unsafe fn destroy(&self, gl: &glow::Context) {
        for framebuffer in self.peel.iter().chain(&self.count).chain([&self.pack]) {
            gl.delete_framebuffer(*framebuffer);
        }
        let textures = self.depth.iter().chain([&self.leaf, &self.colour, &self.inside]).chain(&self.counts);
        for texture in textures {
            gl.delete_texture(*texture);
        }
        for query in &self.queries {
            gl.delete_query(*query);
        }
    }

    /// What this frame peels, from what the last one's layers turned out to
    /// hold -- see [`LayerPlan`].
    unsafe fn plan(&self, gl: &glow::Context) -> LayerPlan {
        let Some(drawn) = self.last.get() else { return LayerPlan::Asking };
        // The queries finish in the order they were sent, so the last one in
        // says they all are.
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

    /// Forget how deep the last boolean went, for a frame that draws none: the
    /// next one may be a different shape altogether.
    pub(super) fn forget(&self) {
        self.last.set(None);
    }
}

impl Gpu {
    /// Make the preview's targets the size of the frame, if it has one to
    /// draw.
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

    /// The section's kept half-space as a shape, made again only when it
    /// changes -- see the module's own note.
    pub(super) fn refresh_half_space(&mut self, request: &Request<'_>) {
        use std::hash::{Hash, Hasher};
        let (Some(csg), Some(plane)) = (&request.live.csg, request.section) else {
            self.csg_half = None;
            return;
        };
        // Big enough for every shape wherever the drag takes it, and changed
        // only when the shapes outgrow it, so a drag does not make a new one on
        // every frame.
        let mut lo = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut hi = -lo;
        for (leaf, moved) in &csg.leaves {
            if let Some(resident) = self.resident.get(&leaf.id) {
                let (a, b) = resident.world_box(&Placing::moved(*moved));
                lo = lo.min(a);
                hi = hi.max(b);
            }
        }
        if lo.x > hi.x {
            self.csg_half = None;
            return;
        }
        let reach = ((hi - lo).length().max(1.0) * 2.0).log2().ceil().exp2();
        let snap = |value: f64| (value / (reach / 4.0)).round() * (reach / 4.0);
        let middle = (lo + hi) * 0.5;
        let centre = Vec3::new(snap(middle.x), snap(middle.y), snap(middle.z));
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for value in [plane.normal.x, plane.normal.y, plane.normal.z, plane.offset, reach, centre.x, centre.y, centre.z]
        {
            value.to_bits().hash(&mut hasher);
        }
        request.palette.cut.hash(&mut hasher);
        let key = hasher.finish();
        if self.csg_half.as_ref().is_some_and(|(held, _)| *held == key) {
            return;
        }
        let cut = request.palette.cut;
        self.csg_half = Some((key, Renderable::surface(half_space(&plane, centre, reach, [cut[0], cut[1], cut[2]]))));
    }

    /// Draw the boolean into the model pass, where the rest of the scene's
    /// faces are already down. Expects the model pass's framebuffer and
    /// leaves it bound, with the model pass's state.
    #[allow(clippy::too_many_arguments)]
    pub(super) unsafe fn draw_csg(
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
        let half = self.csg_half.as_ref().map(|(_, half)| half);
        if let Some(half) = half {
            // The whole, intersected with the half-space kept.
            program.push(leaves.len() as i32);
            program.push(crate::app::CSG_INTERSECTION);
            leaves.push((half, None));
        }
        let drawn: Vec<(&resident::Resident, Placing)> = leaves
            .iter()
            .filter_map(|(leaf, moved)| Some((self.resident.get(&leaf.id)?, Placing::moved(*moved))))
            .collect();
        if drawn.len() != leaves.len() || leaves.len() > MAX_SHAPES {
            return;
        }
        let (width, height) = (targets.width as i32, targets.height as i32);

        gl.disable(glow::CULL_FACE);
        gl.disable(glow::BLEND);
        gl.disable(glow::CLIP_DISTANCE0);
        gl.disable(glow::CLIP_DISTANCE1);
        // The scene's stencil marks the pixels a layer has already drawn, and
        // its `done` target the same for the passes that cannot see the
        // stencil: a pixel done is not peeled or counted again.
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
            gl.begin_query(glow::ANY_SAMPLES_PASSED, targets.queries[layer]);
            for (index, (resident, placing)) in drawn.iter().enumerate() {
                let Some(faces) = &resident.faces else { continue };
                set_projection(gl, peel, resident, view, None, placing);
                if let Some(at) = peel.at("u_leaf") {
                    gl.uniform_1_u32(Some(at), index as u32);
                }
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

            // Which shapes the layer's point is inside, 32 at a time: each
            // shape's faces in front of the layer counted into its own
            // channel, and the counts then packed into bits.
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
                    set_projection(gl, count, resident, view, None, placing);
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
            // Drawn only where no layer has been, and the pixel marked done
            // where it is drawn and where the rest of the scene hides it: the
            // layers behind are hidden there all the more. The reference is
            // what `REPLACE` writes, so "not done" is "not equal to 1".
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
            if let Some(at) = resolve.at("u_tag") {
                gl.uniform_1_u32(Some(at), csg.tag as u32);
            }
            set_i32(gl, resolve, "u_half", if half.is_some() { leaves.len() as i32 - 1 } else { -1 });
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

        // The model pass's state, as the caller left it.
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

/// The side of `plane` it keeps, as a closed box of `reach` round `centre`
/// with one face on the plane, painted `colour`.
fn half_space(plane: &Plane, centre: Vec3, reach: f64, colour: [u8; 3]) -> Mesh {
    let n = plane.normal;
    let foot = centre - n * plane.depth(centre);
    let helper = if n.x.abs() < 0.9 { Vec3::new(1.0, 0.0, 0.0) } else { Vec3::new(0.0, 1.0, 0.0) };
    let u = n.cross(helper).normalized();
    let v = n.cross(u);
    // Kept is where `depth` is negative: behind the plane, against its normal.
    let corner = |i: usize| {
        let a = if i & 1 == 0 { -reach } else { reach };
        let b = if i & 2 == 0 { -reach } else { reach };
        let c = if i & 4 == 0 { 0.0 } else { -2.0 * reach };
        foot + u * a + v * b + n * c
    };
    let mut mesh = Mesh::new();
    // Corners by bit: x = 1, y = 2, back = 4. Each face as two triangles; the
    // counts do not care which way round, and the shading takes either side.
    for [a, b, c, d] in [[0, 1, 3, 2], [4, 6, 7, 5], [0, 4, 5, 1], [2, 3, 7, 6], [0, 2, 6, 4], [1, 5, 7, 3]] {
        mesh.push_triangle(corner(a), corner(b), corner(c));
        mesh.push_triangle(corner(a), corner(c), corner(d));
    }
    mesh.set_tag(simple3d_geom::colour_tag(colour));
    mesh
}
