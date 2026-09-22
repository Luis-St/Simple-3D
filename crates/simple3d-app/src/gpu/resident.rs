//! The meshes the GPU renderer keeps on the card between frames.
//!
//! A renderable is uploaded the first time it is drawn and then left where it
//! is: an orbit, a zoom or a pan changes only the handful of numbers the
//! shaders project it with. What the CPU still hands over each frame is the
//! part of the picture the camera decides -- see [`crate::render::Geometry`].

use super::*;
use crate::raster::Rgba;
use crate::render::{mark_colours, tag_bases, Palette, Renderable, Request, Style, EDGE_BIAS, MARK_BIAS};
use crate::snap::MARK_AXIS;
use crate::view::View;
use eframe::glow::{self, HasContext};
use simple3d_core::config::DisplayMode;
use simple3d_core::scene::Colour;
use simple3d_geom::Vec3;

/// One corner of a face. Every corner of a triangle carries the triangle's own
/// normal, paint and body, so nothing is shared between faces and the shader
/// needs no lookup to cull or shade one.
#[repr(C)]
#[derive(Clone, Copy)]
struct FaceVertex {
    pos: [f32; 3],
    normal: [f32; 3],
    /// The face's paint, or all zero for "the palette's colour".
    colour: [u8; 4],
    body: u32,
}

/// One end of a line, with the other end alongside for the depth bias.
#[repr(C)]
#[derive(Clone, Copy)]
struct LineVertex {
    pos: [f32; 3],
    other: [f32; 3],
    body: u32,
}

/// A vertex buffer and the layout it is drawn with.
pub(super) struct Batch {
    array: glow::VertexArray,
    buffer: glow::Buffer,
    count: i32,
}

/// One renderable, on the card.
pub(crate) struct Resident {
    /// What the positions are stored relative to: the middle of the mesh, so
    /// the single-precision numbers on the card stay small.
    origin: Vec3,
    /// The mesh's extent, relative to `origin`: what the frame's depth range is
    /// sized from, since the card never reports the keys it worked out.
    lo: Vec3,
    hi: Vec3,
    faces: Batch,
    edges: Batch,
    /// Where each principal plane crosses the surface, one run per axis, in
    /// the order `mark_colours` numbers them.
    marks: Batch,
    mark_runs: [(i32, i32); 3],
}

/// What one frame asks of the resident meshes, in the passes that draw it.
#[derive(Default)]
pub(super) struct Plan {
    pub(super) solids: Vec<FaceDraw>,
    pub(super) lines: Vec<LineDraw>,
    pub(super) ghosts: Vec<FaceDraw>,
    pub(super) glows: Vec<FaceDraw>,
}

pub(super) struct FaceDraw {
    id: u64,
    mode: i32,
    base: Rgba,
    tag_base: u16,
}

pub(super) struct LineDraw {
    id: u64,
    what: Lines,
    colour: Rgba,
    bias: f32,
    tag_base: Option<u16>,
}

#[derive(Clone, Copy)]
enum Lines {
    Edges,
    Marks(usize),
}

const SOLID: i32 = 0;
const GHOST: i32 = 1;
const GLOW: i32 = 2;

/// Which resident draws a frame needs: the same choices `render::prepare_with`
/// makes for the steps it leaves out under [`crate::render::Geometry::Resident`].
pub(super) fn plan(request: &Request<'_>) -> Plan {
    let palette: &Palette = &request.palette;
    let mut plan = Plan::default();
    for (item, tag_base) in request.items.iter().zip(tag_bases(&request.items)) {
        let id = item.renderable.id;
        match item.style {
            Style::Solid => {
                let solid = FaceDraw { id, mode: SOLID, base: opaque(palette.solid), tag_base };
                match request.mode {
                    DisplayMode::Wireframe => plan.lines.push(LineDraw {
                        id,
                        what: Lines::Edges,
                        colour: palette.wire,
                        bias: 0.0,
                        tag_base: None,
                    }),
                    DisplayMode::Shaded => plan.solids.push(solid),
                    DisplayMode::ShadedWithEdges => {
                        plan.solids.push(solid);
                        plan.lines.push(LineDraw {
                            id,
                            what: Lines::Edges,
                            colour: palette.edge,
                            bias: EDGE_BIAS,
                            tag_base: Some(tag_base),
                        });
                    }
                }
            }
            Style::Ghost => plan.ghosts.push(FaceDraw { id, mode: GHOST, base: palette.ghost, tag_base: 0 }),
            Style::Glow => plan.glows.push(FaceDraw { id, mode: GLOW, base: palette.glow, tag_base: 0 }),
            Style::Selected => {}
        }
    }
    if request.grid.plane_marks && request.mode != DisplayMode::Wireframe {
        let colours = mark_colours(palette);
        for item in request.items.iter().filter(|item| item.style == Style::Solid) {
            for (axis, colour) in colours.into_iter().enumerate() {
                if request.grid.axes[MARK_AXIS[axis]] {
                    plan.lines.push(LineDraw {
                        id: item.renderable.id,
                        what: Lines::Marks(axis),
                        colour,
                        bias: MARK_BIAS,
                        tag_base: None,
                    });
                }
            }
        }
    }
    plan
}

/// A solid is drawn opaque whatever alpha the palette gives it, as
/// `push_shaded` asks `shade` for.
fn opaque(colour: Rgba) -> Rgba {
    [colour[0], colour[1], colour[2], 255]
}

impl Resident {
    pub(super) unsafe fn upload(gl: &glow::Context, item: &Renderable) -> Result<Resident, String> {
        let mesh = &item.mesh;
        let (lo, hi) = mesh.bounds().unwrap_or((Vec3::ZERO, Vec3::ZERO));
        let origin = (lo + hi) * 0.5;
        let at = |index: u32| {
            let p = mesh.positions[index as usize] - origin;
            [p.x as f32, p.y as f32, p.z as f32]
        };
        let body = |index: u32| item.bodies.get(index as usize).copied().unwrap_or(0) as u32;

        let mut faces = Vec::with_capacity(mesh.indices.len() * 3);
        for (index, tri) in mesh.indices.iter().enumerate() {
            let n = item.normals[index];
            let colour = match Colour::from_tag(mesh.tag(index)) {
                Some(Colour([r, g, b])) => [r, g, b, 255],
                None => [0; 4],
            };
            for &corner in tri {
                faces.push(FaceVertex {
                    pos: at(corner),
                    normal: [n.x as f32, n.y as f32, n.z as f32],
                    colour,
                    body: body(tri[0]),
                });
            }
        }

        let mut edges = Vec::with_capacity(item.edges.len() * 2);
        for &[a, b] in &item.edges {
            edges.push(LineVertex { pos: at(a), other: at(b), body: body(a) });
            edges.push(LineVertex { pos: at(b), other: at(a), body: body(a) });
        }

        let mut marks = Vec::new();
        let mut mark_runs = [(0, 0); 3];
        let relative = |p: Vec3| {
            let p = p - origin;
            [p.x as f32, p.y as f32, p.z as f32]
        };
        for (axis, run) in mark_runs.iter_mut().enumerate() {
            let first = marks.len();
            for &[a, b] in &item.plane_marks[axis] {
                marks.push(LineVertex { pos: relative(a), other: relative(b), body: 0 });
                marks.push(LineVertex { pos: relative(b), other: relative(a), body: 0 });
            }
            *run = (first as i32, (marks.len() - first) as i32);
        }

        Ok(Resident {
            origin,
            lo: lo - origin,
            hi: hi - origin,
            faces: Batch::faces(gl, &faces)?,
            edges: Batch::lines(gl, &edges)?,
            marks: Batch::lines(gl, &marks)?,
            mark_runs,
        })
    }

    pub(super) unsafe fn destroy(&self, gl: &glow::Context) {
        for batch in [&self.faces, &self.edges, &self.marks] {
            gl.delete_vertex_array(batch.array);
            gl.delete_buffer(batch.buffer);
        }
    }

    /// The rows the shaders project this mesh with: screen x, screen y and the
    /// depth key, each a dot product with the stored position plus a constant.
    /// Exactly `View::to_view` followed by `to_vertex`, folded together with
    /// the offset the positions are stored at, in double precision.
    fn rows(&self, view: &View) -> [[f32; 4]; 3] {
        let (right, up) = view.basis();
        let forward = view.forward();
        let s = view.pixels_per_mm();
        let d = self.origin - view.eye();
        let row = |v: Vec3, c: f64| [v.x as f32, v.y as f32, v.z as f32, c as f32];
        [
            row(right * s, view.centre.x as f64 + d.dot(right) * s),
            row(-(up * s), view.centre.y as f64 - d.dot(up) * s),
            row(-forward, -d.dot(forward)),
        ]
    }

    /// The section plane as the shaders test it: a distance from the stored
    /// position that is positive where the model is kept.
    fn clip(&self, section: Option<simple3d_geom::section::Plane>) -> [f32; 4] {
        match section {
            // `Plane::depth` is negative on the kept side, so this is its
            // negation, moved to the stored positions' origin.
            Some(plane) => {
                let n = plane.normal;
                [-n.x as f32, -n.y as f32, -n.z as f32, (plane.offset - n.dot(self.origin)) as f32]
            }
            None => [0.0, 0.0, 0.0, 1.0],
        }
    }

    /// The depth keys this mesh can reach under `view`: its box's corners.
    pub(super) fn keys(&self, view: &View) -> impl Iterator<Item = f32> {
        let [_, _, key] = self.rows(view);
        let (lo, hi) = (self.lo, self.hi);
        (0..8).map(move |corner| {
            let x = if corner & 1 == 0 { lo.x } else { hi.x };
            let y = if corner & 2 == 0 { lo.y } else { hi.y };
            let z = if corner & 4 == 0 { lo.z } else { hi.z };
            key[0] * x as f32 + key[1] * y as f32 + key[2] * z as f32 + key[3]
        })
    }
}

impl Batch {
    unsafe fn faces(gl: &glow::Context, vertices: &[FaceVertex]) -> Result<Batch, String> {
        let batch = Batch::new(gl, bytes_of(vertices), vertices.len())?;
        let stride = std::mem::size_of::<FaceVertex>() as i32;
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, stride, 12);
        gl.enable_vertex_attrib_array(2);
        gl.vertex_attrib_pointer_f32(2, 4, glow::UNSIGNED_BYTE, true, stride, 24);
        gl.enable_vertex_attrib_array(3);
        gl.vertex_attrib_pointer_i32(3, 1, glow::UNSIGNED_INT, stride, 28);
        gl.bind_vertex_array(None);
        Ok(batch)
    }

    unsafe fn lines(gl: &glow::Context, vertices: &[LineVertex]) -> Result<Batch, String> {
        let batch = Batch::new(gl, bytes_of(vertices), vertices.len())?;
        let stride = std::mem::size_of::<LineVertex>() as i32;
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, stride, 12);
        gl.enable_vertex_attrib_array(2);
        gl.vertex_attrib_pointer_i32(2, 1, glow::UNSIGNED_INT, stride, 24);
        gl.bind_vertex_array(None);
        Ok(batch)
    }

    /// Make the array and buffer and fill the buffer, leaving both bound for
    /// the caller to describe the layout.
    unsafe fn new(gl: &glow::Context, bytes: &[u8], count: usize) -> Result<Batch, String> {
        let array = gl.create_vertex_array()?;
        let buffer = gl.create_buffer()?;
        gl.bind_vertex_array(Some(array));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(buffer));
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::STATIC_DRAW);
        Ok(Batch { array, buffer, count: count as i32 })
    }
}

fn bytes_of<T: Copy>(vertices: &[T]) -> &[u8] {
    // Both vertex types are `repr(C)` and hold only numbers.
    unsafe { std::slice::from_raw_parts(vertices.as_ptr() as *const u8, std::mem::size_of_val(vertices)) }
}

impl Gpu {
    /// Upload whatever `plan` draws that is not on the card yet, and let go of
    /// what the frame no longer draws -- a renderable that has been replaced
    /// will not be asked for again, and holding on to it would hold its whole
    /// mesh in video memory.
    pub(super) unsafe fn keep_resident(&mut self, gl: &glow::Context, request: &Request<'_>) -> Result<(), String> {
        let wanted: std::collections::HashSet<u64> = request.items.iter().map(|item| item.renderable.id).collect();
        let stale: Vec<u64> = self.resident.keys().copied().filter(|id| !wanted.contains(id)).collect();
        for id in stale {
            if let Some(resident) = self.resident.remove(&id) {
                resident.destroy(gl);
            }
        }
        for item in &request.items {
            if let std::collections::hash_map::Entry::Vacant(slot) = self.resident.entry(item.renderable.id) {
                slot.insert(Resident::upload(gl, item.renderable)?);
            }
        }
        Ok(())
    }

    /// Faces from the card, in whichever pass the caller has set up.
    pub(super) unsafe fn draw_faces(
        &self,
        gl: &glow::Context,
        draws: &[FaceDraw],
        view: &View,
        section: Option<simple3d_geom::section::Plane>,
        viewport: [f32; 2],
        depth: [f32; 2],
    ) {
        if draws.is_empty() {
            return;
        }
        let program = &self.faces;
        gl.use_program(Some(program.program));
        gl.enable(glow::CLIP_DISTANCE0);
        set2(gl, program, "u_viewport", viewport);
        set2(gl, program, "u_depth", depth);
        let forward = view.forward();
        if let Some(at) = program.at("u_forward") {
            gl.uniform_3_f32(Some(at), forward.x as f32, forward.y as f32, forward.z as f32);
        }
        for draw in draws {
            let Some(resident) = self.resident.get(&draw.id) else { continue };
            set_projection(gl, program, resident, view, section);
            let base = draw.base.map(|c| c as f32);
            if let Some(at) = program.at("u_base") {
                gl.uniform_4_f32_slice(Some(at), &base);
            }
            if let Some(at) = program.at("u_mode") {
                gl.uniform_1_i32(Some(at), draw.mode);
            }
            if let Some(at) = program.at("u_tag_base") {
                gl.uniform_1_u32(Some(at), draw.tag_base as u32);
            }
            gl.bind_vertex_array(Some(resident.faces.array));
            gl.draw_arrays(glow::TRIANGLES, 0, resident.faces.count);
        }
        gl.disable(glow::CLIP_DISTANCE0);
        gl.bind_vertex_array(Some(self.buffer.array));
    }

    /// Edges, the wireframe and plane marks from the card.
    pub(super) unsafe fn draw_lines(
        &self,
        gl: &glow::Context,
        draws: &[LineDraw],
        view: &View,
        section: Option<simple3d_geom::section::Plane>,
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
            set_projection(gl, program, resident, view, section);
            if let Some(at) = program.at("u_colour") {
                gl.uniform_4_f32_slice(Some(at), &as_float(draw.colour));
            }
            if let Some(at) = program.at("u_bias") {
                gl.uniform_1_f32(Some(at), draw.bias);
            }
            if let Some(at) = program.at("u_tagged") {
                gl.uniform_1_i32(Some(at), draw.tag_base.is_some() as i32);
            }
            if let Some(at) = program.at("u_tag_base") {
                gl.uniform_1_u32(Some(at), draw.tag_base.unwrap_or(0) as u32);
            }
            let (batch, first, count) = match draw.what {
                Lines::Edges => (&resident.edges, 0, resident.edges.count),
                Lines::Marks(axis) => (&resident.marks, resident.mark_runs[axis].0, resident.mark_runs[axis].1),
            };
            if count > 0 {
                gl.bind_vertex_array(Some(batch.array));
                gl.draw_arrays(glow::LINES, first, count);
            }
        }
        gl.disable(glow::CLIP_DISTANCE0);
        gl.bind_vertex_array(Some(self.buffer.array));
    }

    /// Widen the frame's depth range to take in every resident mesh it draws,
    /// with room for the largest bias a line on one gets.
    pub(super) fn see_resident(&self, request: &Request<'_>, passes: &mut Passes) {
        for item in &request.items {
            let Some(resident) = self.resident.get(&item.renderable.id) else { continue };
            for key in resident.keys(&request.view) {
                passes.saw(key);
                passes.saw(key + MARK_BIAS * key.abs());
            }
        }
    }
}

unsafe fn set2(gl: &glow::Context, program: &Program, name: &str, value: [f32; 2]) {
    if let Some(at) = program.at(name) {
        gl.uniform_2_f32(Some(at), value[0], value[1]);
    }
}

unsafe fn set_projection(
    gl: &glow::Context,
    program: &Program,
    resident: &Resident,
    view: &View,
    section: Option<simple3d_geom::section::Plane>,
) {
    let rows = resident.rows(view);
    for (name, row) in ["u_row_x", "u_row_y", "u_row_key"].into_iter().zip(rows) {
        if let Some(at) = program.at(name) {
            gl.uniform_4_f32_slice(Some(at), &row);
        }
    }
    if let Some(at) = program.at("u_clip") {
        gl.uniform_4_f32_slice(Some(at), &resident.clip(section));
    }
}
