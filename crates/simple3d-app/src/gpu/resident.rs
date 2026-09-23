//! The meshes the GPU renderer keeps on the card between frames.
//!
//! A renderable is uploaded the first time it is drawn and then left where it
//! is: an orbit, a zoom or a pan changes only the handful of numbers the
//! shaders project it with. Everything a mesh is drawn with that does not
//! depend on the camera -- its faces, its edges, its outline, where a plane
//! crosses it, the cap a section leaves -- is worked out on the card from what
//! is uploaded here, so a frame hands the card a camera and nothing per
//! triangle at all.
//!
//! What is uploaded is the mesh as it is: one position per welded vertex, the
//! triangles and the edges as index lists straight out of the renderable, and
//! the body of each vertex. Only the positions are rewritten on the way, into
//! single precision relative to the mesh's centre. Each part is made the
//! first time a frame asks for it, so a body that is only ever outlined never
//! has its faces uploaded, and one that is only drawn never has its outline.

use super::*;
use crate::raster::Rgba;
use crate::render::{
    mark_colours, shade, tag_bases, to_vertex, Palette, Renderable, Request, Style, EDGE_BIAS, EDGE_ON, MARK_BIAS,
    SELECTION_BIAS, SELECTION_CREASE,
};
use crate::snap::MARK_AXIS;
use crate::view::View;
use eframe::glow::{self, HasContext};
use simple3d_core::config::DisplayMode;
use simple3d_geom::section::Plane;
use simple3d_geom::Vec3;

/// A vertex array and the element or vertex buffer it draws from.
pub(super) struct Batch {
    array: glow::VertexArray,
    buffer: glow::Buffer,
    count: i32,
}

/// The mesh's vertices: position and body, one of each per welded vertex.
struct Vertices {
    positions: glow::Buffer,
    bodies: glow::Buffer,
}

/// The mesh laid out in textures, for a geometry stage that has to look up a
/// triangle other than the one it was handed -- the outline, which asks about
/// both faces along an edge. See `TABLES_COMMON`.
struct Tables {
    positions: glow::Texture,
    triangles: glow::Texture,
    bodies: glow::Texture,
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
    vertices: Option<Vertices>,
    faces: Option<Batch>,
    /// One colour tag per triangle, when any triangle is painted.
    paint: Option<glow::Texture>,
    edges: Option<Batch>,
    tables: Option<Tables>,
    outline: Option<Batch>,
}

/// Which parts of a resident a frame draws with.
#[derive(Clone, Copy, Default)]
pub(super) struct Needs {
    faces: bool,
    edges: bool,
    outline: bool,
}

/// What one frame asks of the resident meshes, in the passes that draw it.
#[derive(Default)]
pub(super) struct Plan {
    pub(super) solids: Vec<FaceDraw>,
    pub(super) lines: Vec<LineDraw>,
    pub(super) ghosts: Vec<FaceDraw>,
    pub(super) glows: Vec<FaceDraw>,
    /// Selected and glowing bodies' outlines.
    pub(super) outlines: Vec<OutlineDraw>,
    /// Plane marks and the edge round a section's cut.
    pub(super) crossings: Vec<CrossingDraw>,
    /// A section's cap, filled where the cut runs through material.
    pub(super) caps: Vec<CapDraw>,
}

pub(super) struct FaceDraw {
    id: u64,
    mode: i32,
    base: Rgba,
    tag_base: u16,
}

pub(super) struct LineDraw {
    id: u64,
    colour: Rgba,
    bias: f32,
    tag_base: Option<u16>,
}

pub(super) struct OutlineDraw {
    id: u64,
    colour: Rgba,
    tag_base: u16,
    /// Every crease rather than only those facing the eye: wireframe.
    all_creases: bool,
}

pub(super) struct CrossingDraw {
    id: u64,
    /// Each plane, and the colour its crossing is drawn in.
    planes: Vec<(Plane, Rgba)>,
    /// Whether the crossing is cut by the section like everything on the
    /// model. The edge round the cut lies in the section plane itself, where
    /// the clip would only fray it.
    clipped: bool,
}

pub(super) struct CapDraw {
    id: u64,
    pub(super) plane: Plane,
    pub(super) colour: Rgba,
    /// The polygon the plane leaves in the mesh's box, projected: the cap is
    /// this, wherever the cut runs through material.
    pub(super) fill: Vec<GpuVertex>,
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
                    DisplayMode::Wireframe => {
                        plan.lines.push(LineDraw { id, colour: palette.wire, bias: 0.0, tag_base: None })
                    }
                    DisplayMode::Shaded => plan.solids.push(solid),
                    DisplayMode::ShadedWithEdges => {
                        plan.solids.push(solid);
                        plan.lines.push(LineDraw {
                            id,
                            colour: palette.edge,
                            bias: EDGE_BIAS,
                            tag_base: Some(tag_base),
                        });
                    }
                }
                // The cut filled in wherever the mode fills anything, and the
                // line round it wherever the mode draws the model's lines --
                // `push_cap`'s two rules.
                if let Some(plane) = request.section {
                    if request.mode != DisplayMode::Wireframe {
                        let colour = shade(palette.cut, plane.normal, request.view.forward(), 255);
                        plan.caps.push(CapDraw { id, plane, colour, fill: Vec::new() });
                    }
                    if request.mode != DisplayMode::Shaded {
                        let colour = match request.mode {
                            DisplayMode::Wireframe => palette.wire,
                            _ => palette.edge,
                        };
                        plan.crossings.push(CrossingDraw { id, planes: vec![(plane, colour)], clipped: false });
                    }
                }
            }
            Style::Ghost => plan.ghosts.push(FaceDraw { id, mode: GHOST, base: palette.ghost, tag_base: 0 }),
            Style::Glow | Style::Selected => {
                if item.style == Style::Glow {
                    plan.glows.push(FaceDraw { id, mode: GLOW, base: palette.glow, tag_base: 0 });
                }
                // Prepared without the adjacency, a body has only its creases
                // to be outlined by -- `push_selection`'s fallback.
                if item.renderable.outline.is_empty() {
                    plan.lines.push(LineDraw {
                        id,
                        colour: palette.selected,
                        bias: SELECTION_BIAS,
                        tag_base: Some(tag_base),
                    });
                } else {
                    plan.outlines.push(OutlineDraw {
                        id,
                        colour: palette.selected,
                        tag_base,
                        all_creases: request.mode == DisplayMode::Wireframe,
                    });
                }
            }
        }
    }
    if request.grid.plane_marks && request.mode != DisplayMode::Wireframe {
        let colours = mark_colours(palette);
        let planes: Vec<(Plane, Rgba)> = (0..3)
            .filter(|&axis| request.grid.axes[MARK_AXIS[axis]])
            .map(|axis| (Plane::on_axis(axis, 0.0, false), colours[axis]))
            .collect();
        if !planes.is_empty() {
            for item in request.items.iter().filter(|item| item.style == Style::Solid) {
                plan.crossings.push(CrossingDraw { id: item.renderable.id, planes: planes.clone(), clipped: true });
            }
        }
    }
    plan
}

impl Plan {
    /// What each resident has to have on the card for this plan.
    fn needs(&self) -> std::collections::HashMap<u64, Needs> {
        let mut needs: std::collections::HashMap<u64, Needs> = std::collections::HashMap::new();
        let faces = self.solids.iter().chain(&self.ghosts).chain(&self.glows).map(|draw| draw.id);
        let faces = faces.chain(self.crossings.iter().map(|draw| draw.id)).chain(self.caps.iter().map(|draw| draw.id));
        for id in faces {
            needs.entry(id).or_default().faces = true;
        }
        for draw in &self.lines {
            needs.entry(draw.id).or_default().edges = true;
        }
        for draw in &self.outlines {
            needs.entry(draw.id).or_default().outline = true;
        }
        needs
    }
}

/// A solid is drawn opaque whatever alpha the palette gives it, as
/// `push_shaded` asks `shade` for.
fn opaque(colour: Rgba) -> Rgba {
    [colour[0], colour[1], colour[2], 255]
}

impl Resident {
    fn new(item: &Renderable) -> Resident {
        let (lo, hi) = item.mesh.bounds().unwrap_or((Vec3::ZERO, Vec3::ZERO));
        let origin = (lo + hi) * 0.5;
        Resident {
            origin,
            lo: lo - origin,
            hi: hi - origin,
            vertices: None,
            faces: None,
            paint: None,
            edges: None,
            tables: None,
            outline: None,
        }
    }

    /// Upload whatever of `item` `needs` asks for that is not on the card yet.
    unsafe fn ensure(
        &mut self,
        gl: &glow::Context,
        item: &Renderable,
        needs: Needs,
        width: usize,
    ) -> Result<(), String> {
        let wants_vertices = (needs.faces && self.faces.is_none()) || (needs.edges && self.edges.is_none());
        let wants_tables = needs.outline && self.tables.is_none();
        if !wants_vertices && !wants_tables {
            return Ok(());
        }
        let origin = self.origin;
        let positions: Vec<[f32; 3]> = crate::render::map_in_order(item.mesh.positions.len(), |index| {
            let p = item.mesh.positions[index] - origin;
            [p.x as f32, p.y as f32, p.z as f32]
        });
        if wants_vertices && self.vertices.is_none() {
            self.vertices = Some(Vertices {
                positions: buffer(gl, glow::ARRAY_BUFFER, bytes_of(&positions))?,
                bodies: buffer(gl, glow::ARRAY_BUFFER, bytes_of(&item.bodies))?,
            });
        }
        let vertices = self.vertices.as_ref();
        if needs.faces && self.faces.is_none() {
            let vertices = vertices.expect("made above");
            self.faces = Some(indexed(gl, vertices, bytes_of(&item.mesh.indices), item.mesh.indices.len() as i32 * 3)?);
            let tags = &item.mesh.tags;
            if tags.len() == item.mesh.indices.len() && tags.iter().any(|&tag| simple3d_geom::tag_colour(tag).is_some())
            {
                self.paint = Some(table(gl, width, Format::U32, bytes_of(tags), tags.len())?);
            }
        }
        if needs.edges && self.edges.is_none() {
            let vertices = vertices.expect("made above");
            self.edges = Some(indexed(gl, vertices, bytes_of(&item.edges), item.edges.len() as i32 * 2)?);
        }
        if needs.outline && self.outline.is_none() {
            if self.tables.is_none() {
                self.tables = Some(Tables {
                    positions: table(gl, width, Format::Vec3, bytes_of(&positions), positions.len())?,
                    triangles: table(gl, width, Format::UVec3, bytes_of(&item.mesh.indices), item.mesh.indices.len())?,
                    bodies: table(gl, width, Format::U16, bytes_of(&item.bodies), item.bodies.len())?,
                });
            }
            // A junction is inside the shape, never on its outline -- see
            // `BorderEdge::junction` -- so it is left behind here rather than
            // asked about on every frame.
            let edges: Vec<[u32; 4]> = item
                .outline
                .iter()
                .filter(|edge| !edge.junction)
                .map(|edge| [edge.ends[0], edge.ends[1], edge.faces[0], edge.faces[1]])
                .collect();
            let array = gl.create_vertex_array()?;
            gl.bind_vertex_array(Some(array));
            let buffer = buffer(gl, glow::ARRAY_BUFFER, bytes_of(&edges))?;
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_i32(0, 4, glow::UNSIGNED_INT, 16, 0);
            gl.bind_vertex_array(None);
            self.outline = Some(Batch { array, buffer, count: edges.len() as i32 });
        }
        Ok(())
    }

    pub(super) unsafe fn destroy(&self, gl: &glow::Context) {
        for batch in [&self.faces, &self.edges, &self.outline].into_iter().flatten() {
            gl.delete_vertex_array(batch.array);
            gl.delete_buffer(batch.buffer);
        }
        if let Some(vertices) = &self.vertices {
            gl.delete_buffer(vertices.positions);
            gl.delete_buffer(vertices.bodies);
        }
        if let Some(tables) = &self.tables {
            for texture in [tables.positions, tables.triangles, tables.bodies] {
                gl.delete_texture(texture);
            }
        }
        if let Some(paint) = self.paint {
            gl.delete_texture(paint);
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

    /// A plane as the shaders test it: a distance from the stored position
    /// that is positive where `Plane::depth` is.
    fn plane(&self, plane: &Plane) -> [f32; 4] {
        let n = plane.normal;
        [n.x as f32, n.y as f32, n.z as f32, (n.dot(self.origin) - plane.offset) as f32]
    }

    /// The section plane as the shaders clip with it: positive where the model
    /// is kept, which is where `Plane::depth` is negative.
    fn clip(&self, section: Option<Plane>) -> [f32; 4] {
        match section {
            Some(plane) => self.plane(&plane).map(|value| -value),
            None => [0.0, 0.0, 0.0, 1.0],
        }
    }

    /// The largest coordinate a stored position has, which is what single
    /// precision loses its last bits relative to.
    fn extent(&self) -> f64 {
        let lo = self.lo.x.abs().max(self.lo.y.abs()).max(self.lo.z.abs());
        let hi = self.hi.x.abs().max(self.hi.y.abs()).max(self.hi.z.abs());
        lo.max(hi)
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

    /// The polygon `plane` leaves in this mesh's box, in world space and in
    /// order round it: everything the cap can cover. The box is taken a hair
    /// larger than the mesh, so a cut lying exactly on one of its faces -- a
    /// section at the base of a shape standing on the ground -- still meets it.
    pub(super) fn cap_polygon(&self, plane: &Plane) -> Vec<Vec3> {
        let margin = Vec3::new(1.0, 1.0, 1.0) * (self.extent() * 1e-3 + 1e-6);
        let (lo, hi) = (self.origin + self.lo - margin, self.origin + self.hi + margin);
        let corner = |index: usize| {
            Vec3::new(
                if index & 1 == 0 { lo.x } else { hi.x },
                if index & 2 == 0 { lo.y } else { hi.y },
                if index & 4 == 0 { lo.z } else { hi.z },
            )
        };
        let mut points: Vec<Vec3> = Vec::new();
        for a in 0..8 {
            for bit in [1, 2, 4] {
                let b = a | bit;
                if b == a {
                    continue;
                }
                let (pa, pb) = (corner(a), corner(b));
                let (da, db) = (plane.depth(pa), plane.depth(pb));
                if (da <= 0.0) != (db <= 0.0) {
                    points.push(pa + (pb - pa) * (da / (da - db)));
                }
            }
        }
        if points.len() < 3 {
            return Vec::new();
        }
        let centre = points.iter().fold(Vec3::ZERO, |sum, &p| sum + p) / points.len() as f64;
        let u = (points[0] - centre).normalized();
        let v = plane.normal.cross(u);
        points.sort_by(|a, b| {
            let angle = |p: &Vec3| (*p - centre).dot(v).atan2((*p - centre).dot(u));
            angle(a).partial_cmp(&angle(b)).unwrap_or(std::cmp::Ordering::Equal)
        });
        points
    }
}

/// How a table's texels are laid out.
#[derive(Clone, Copy)]
enum Format {
    Vec3,
    UVec3,
    U16,
    U32,
}

impl Format {
    /// Internal format, pixel format, component type and bytes per texel.
    fn gl(self) -> (u32, u32, u32, usize) {
        match self {
            Format::Vec3 => (glow::RGB32F, glow::RGB, glow::FLOAT, 12),
            Format::UVec3 => (glow::RGB32UI, glow::RGB_INTEGER, glow::UNSIGNED_INT, 12),
            Format::U16 => (glow::R16UI, glow::RED_INTEGER, glow::UNSIGNED_SHORT, 2),
            Format::U32 => (glow::R32UI, glow::RED_INTEGER, glow::UNSIGNED_INT, 4),
        }
    }
}

/// `count` texels of `bytes` as a texture `width` texels wide, filled row by
/// row: what a shader reads by index with `cell`. The rows are handed over
/// straight from the slice, the last, short one on its own, so nothing is
/// copied to pad it out.
unsafe fn table(
    gl: &glow::Context,
    width: usize,
    format: Format,
    bytes: &[u8],
    count: usize,
) -> Result<glow::Texture, String> {
    let (internal, pixels, kind, texel) = format.gl();
    let rows = count.div_ceil(width).max(1);
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
    gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
    let upload_width = if rows == 1 { count.max(1) } else { width };
    gl.tex_image_2d(
        glow::TEXTURE_2D,
        0,
        internal as i32,
        upload_width as i32,
        rows as i32,
        0,
        pixels,
        kind,
        glow::PixelUnpackData::Slice(None),
    );
    let full = count / upload_width;
    if full > 0 {
        let data = &bytes[..full * upload_width * texel];
        let unpack = glow::PixelUnpackData::Slice(Some(data));
        gl.tex_sub_image_2d(glow::TEXTURE_2D, 0, 0, 0, upload_width as i32, full as i32, pixels, kind, unpack);
    }
    let rest = count - full * upload_width;
    if rest > 0 {
        let data = &bytes[full * upload_width * texel..count * texel];
        let unpack = glow::PixelUnpackData::Slice(Some(data));
        gl.tex_sub_image_2d(glow::TEXTURE_2D, 0, 0, full as i32, rest as i32, 1, pixels, kind, unpack);
    }
    gl.bind_texture(glow::TEXTURE_2D, None);
    Ok(texture)
}

/// A buffer filled with `bytes`, left bound to `target`.
unsafe fn buffer(gl: &glow::Context, target: u32, bytes: &[u8]) -> Result<glow::Buffer, String> {
    let buffer = gl.create_buffer()?;
    gl.bind_buffer(target, Some(buffer));
    gl.buffer_data_u8_slice(target, bytes, glow::STATIC_DRAW);
    Ok(buffer)
}

/// A vertex array over the mesh's vertices, drawing the elements in
/// `indices`.
unsafe fn indexed(gl: &glow::Context, vertices: &Vertices, indices: &[u8], count: i32) -> Result<Batch, String> {
    let array = gl.create_vertex_array()?;
    gl.bind_vertex_array(Some(array));
    gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertices.positions));
    gl.enable_vertex_attrib_array(0);
    gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, 12, 0);
    gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertices.bodies));
    gl.enable_vertex_attrib_array(1);
    gl.vertex_attrib_pointer_i32(1, 1, glow::UNSIGNED_SHORT, 2, 0);
    // Bound while the array is: the element buffer is part of its state.
    let buffer = buffer(gl, glow::ELEMENT_ARRAY_BUFFER, indices)?;
    gl.bind_vertex_array(None);
    Ok(Batch { array, buffer, count })
}

fn bytes_of<T: Copy>(values: &[T]) -> &[u8] {
    // Only ever handed plain numbers and arrays of them, which have no padding.
    unsafe { std::slice::from_raw_parts(values.as_ptr() as *const u8, std::mem::size_of_val(values)) }
}

impl Gpu {
    /// Upload whatever `plan` draws that is not on the card yet, and let go of
    /// what the frame no longer draws -- a renderable that has been replaced
    /// will not be asked for again, and holding on to it would hold its whole
    /// mesh in video memory.
    pub(super) unsafe fn keep_resident(
        &mut self,
        gl: &glow::Context,
        request: &Request<'_>,
        plan: &Plan,
    ) -> Result<(), String> {
        let wanted: std::collections::HashSet<u64> = request.items.iter().map(|item| item.renderable.id).collect();
        let stale: Vec<u64> = self.resident.keys().copied().filter(|id| !wanted.contains(id)).collect();
        for id in stale {
            if let Some(resident) = self.resident.remove(&id) {
                resident.destroy(gl);
            }
        }
        let needs = plan.needs();
        for item in &request.items {
            let id = item.renderable.id;
            let resident = self.resident.entry(id).or_insert_with(|| Resident::new(item.renderable));
            let need = needs.get(&id).copied().unwrap_or_default();
            resident.ensure(gl, item.renderable, need, self.table_width)?;
        }
        Ok(())
    }

    /// Work out, for each cap in `plan`, the polygon it is filled over, and
    /// widen the frame's depth range to take it in.
    pub(super) fn place_caps(&self, view: &View, plan: &mut Plan, passes: &mut Passes) {
        for cap in &mut plan.caps {
            let Some(resident) = self.resident.get(&cap.id) else { continue };
            let polygon = resident.cap_polygon(&cap.plane);
            let projected: Vec<_> = polygon.iter().map(|&p| to_vertex(view, view.to_view(p))).collect();
            for vertex in &projected {
                passes.saw(vertex.key);
            }
            for index in 1..projected.len().saturating_sub(1) {
                for vertex in [projected[0], projected[index], projected[index + 1]] {
                    cap.fill.push(GpuVertex::new(vertex, cap.colour, 0, 0));
                }
            }
        }
    }

    /// Faces from the card, in whichever pass the caller has set up.
    #[allow(clippy::too_many_arguments)]
    pub(super) unsafe fn draw_faces(
        &self,
        gl: &glow::Context,
        draws: &[FaceDraw],
        view: &View,
        section: Option<Plane>,
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
        set_forward(gl, program, view);
        set_i32(gl, program, "u_table_width", self.table_width as i32);
        set_i32(gl, program, "u_paint", 0);
        for draw in draws {
            let Some(resident) = self.resident.get(&draw.id) else { continue };
            let Some(faces) = &resident.faces else { continue };
            set_projection(gl, program, resident, view, section);
            if let Some(at) = program.at("u_base") {
                gl.uniform_4_f32_slice(Some(at), &draw.base.map(|c| c as f32));
            }
            set_i32(gl, program, "u_mode", draw.mode);
            if let Some(at) = program.at("u_tag_base") {
                gl.uniform_1_u32(Some(at), draw.tag_base as u32);
            }
            set_i32(gl, program, "u_painted", resident.paint.is_some() as i32);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, resident.paint);
            // A solid's far side is never seen, so it is culled -- by the
            // pipeline, which asks exactly `push_shaded`'s question. A ghost
            // and a glow show the whole shell.
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
        gl.bind_texture(glow::TEXTURE_2D, None);
        gl.bind_vertex_array(Some(self.buffer.array));
    }

    /// Feature edges and the wireframe from the card.
    pub(super) unsafe fn draw_lines(
        &self,
        gl: &glow::Context,
        draws: &[LineDraw],
        view: &View,
        section: Option<Plane>,
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
            set_projection(gl, program, resident, view, section);
            if let Some(at) = program.at("u_colour") {
                gl.uniform_4_f32_slice(Some(at), &as_float(draw.colour));
            }
            if let Some(at) = program.at("u_bias") {
                gl.uniform_1_f32(Some(at), draw.bias);
            }
            set_i32(gl, program, "u_tagged", draw.tag_base.is_some() as i32);
            if let Some(at) = program.at("u_tag_base") {
                gl.uniform_1_u32(Some(at), draw.tag_base.unwrap_or(0) as u32);
            }
            gl.bind_vertex_array(Some(edges.array));
            gl.draw_elements(glow::LINES, edges.count, glow::UNSIGNED_INT, 0);
        }
        gl.disable(glow::CLIP_DISTANCE0);
        gl.bind_vertex_array(Some(self.buffer.array));
    }

    /// Selected and glowing bodies' outlines, found on the card each frame.
    pub(super) unsafe fn draw_outlines(
        &self,
        gl: &glow::Context,
        draws: &[OutlineDraw],
        view: &View,
        section: Option<Plane>,
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
        if let Some(at) = program.at("u_edge_on") {
            gl.uniform_1_f32(Some(at), EDGE_ON as f32);
        }
        if let Some(at) = program.at("u_crease") {
            gl.uniform_1_f32(Some(at), SELECTION_CREASE.to_radians().cos() as f32);
        }
        if let Some(at) = program.at("u_bias") {
            gl.uniform_1_f32(Some(at), SELECTION_BIAS);
        }
        for (unit, name) in ["u_positions", "u_triangles", "u_bodies"].into_iter().enumerate() {
            set_i32(gl, program, name, unit as i32);
        }
        for draw in draws {
            let Some(resident) = self.resident.get(&draw.id) else { continue };
            let (Some(outline), Some(tables)) = (&resident.outline, &resident.tables) else { continue };
            if outline.count == 0 {
                continue;
            }
            set_projection(gl, program, resident, view, section);
            if let Some(at) = program.at("u_colour") {
                gl.uniform_4_f32_slice(Some(at), &as_float(draw.colour));
            }
            if let Some(at) = program.at("u_tag_base") {
                gl.uniform_1_u32(Some(at), draw.tag_base as u32);
            }
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

    /// Where planes cross the resident meshes: plane marks, and the edge round
    /// a section's cut.
    pub(super) unsafe fn draw_crossings(
        &self,
        gl: &glow::Context,
        draws: &[CrossingDraw],
        view: &View,
        section: Option<Plane>,
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
        if let Some(at) = program.at("u_bias") {
            gl.uniform_1_f32(Some(at), MARK_BIAS);
        }
        for draw in draws {
            let Some(resident) = self.resident.get(&draw.id) else { continue };
            let Some(faces) = &resident.faces else { continue };
            set_projection(gl, program, resident, view, section.filter(|_| draw.clipped));
            let planes: Vec<[f32; 4]> = draw.planes.iter().map(|(plane, _)| resident.plane(plane)).collect();
            let colours: Vec<[f32; 4]> = draw.planes.iter().map(|&(_, colour)| as_float(colour)).collect();
            set_i32(gl, program, "u_planes", planes.len() as i32);
            if let Some(at) = program.at("u_plane[0]") {
                gl.uniform_4_f32_slice(Some(at), planes.as_flattened());
            }
            if let Some(at) = program.at("u_plane_colour[0]") {
                gl.uniform_4_f32_slice(Some(at), colours.as_flattened());
            }
            // A few times what single precision loses on the largest number in
            // play: the stored coordinates, and the plane's own offset.
            let offset = planes.iter().fold(0.0_f64, |most, plane| most.max(plane[3].abs() as f64));
            if let Some(at) = program.at("u_on_plane") {
                gl.uniform_1_f32(Some(at), ((resident.extent() + offset) * 4e-7 + 1e-9) as f32);
            }
            gl.bind_vertex_array(Some(faces.array));
            gl.draw_elements(glow::TRIANGLES, faces.count, glow::UNSIGNED_INT, 0);
        }
        gl.disable(glow::CLIP_DISTANCE0);
        gl.bind_vertex_array(Some(self.buffer.array));
    }

    /// A section's caps: for each, the stencil counts how often the cut
    /// surface winds round each pixel -- every face of the kept part, front
    /// faces up and back faces down -- and the polygon the plane leaves in the
    /// mesh's box is filled wherever the count is not zero, which is wherever
    /// the plane runs through material. `section::loops` and `section::fill`
    /// find the same region on the CPU by chaining the cut into outlines; the
    /// winding count needs no outlines, and so no pass over the mesh at all.
    ///
    /// Winding rather than parity, as `fill` reads its outlines: two bodies of
    /// one mesh that overlap are both material, not a hole where they meet.
    ///
    /// Expects the model pass's state, and leaves it as it found it.
    #[allow(clippy::too_many_arguments)]
    pub(super) unsafe fn draw_caps(
        &self,
        gl: &glow::Context,
        draws: &[CapDraw],
        view: &View,
        section: Option<Plane>,
        viewport: [f32; 2],
        depth: [f32; 2],
    ) {
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
            gl.stencil_func(glow::ALWAYS, 0, 0xFF);
            gl.stencil_op_separate(glow::FRONT, glow::KEEP, glow::KEEP, glow::INCR_WRAP);
            gl.stencil_op_separate(glow::BACK, glow::KEEP, glow::KEEP, glow::DECR_WRAP);

            let program = &self.faces;
            gl.use_program(Some(program.program));
            gl.enable(glow::CLIP_DISTANCE0);
            set2(gl, program, "u_viewport", viewport);
            set2(gl, program, "u_depth", depth);
            set_projection(gl, program, resident, view, section);
            set_i32(gl, program, "u_mode", GHOST);
            set_i32(gl, program, "u_painted", 0);
            gl.bind_vertex_array(Some(faces.array));
            gl.draw_elements(glow::TRIANGLES, faces.count, glow::UNSIGNED_INT, 0);
            gl.disable(glow::CLIP_DISTANCE0);

            gl.color_mask(true, true, true, true);
            gl.depth_mask(true);
            gl.enable(glow::DEPTH_TEST);
            gl.stencil_func(glow::NOTEQUAL, 0, 0xFF);
            gl.stencil_op(glow::KEEP, glow::KEEP, glow::KEEP);
            gl.use_program(Some(self.solid.program));
            gl.bind_vertex_array(Some(self.buffer.array));
            self.batch(gl, glow::TRIANGLES, &cap.fill);
            gl.disable(glow::STENCIL_TEST);
        }
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

unsafe fn set_i32(gl: &glow::Context, program: &Program, name: &str, value: i32) {
    if let Some(at) = program.at(name) {
        gl.uniform_1_i32(Some(at), value);
    }
}

unsafe fn set_forward(gl: &glow::Context, program: &Program, view: &View) {
    let forward = view.forward();
    if let Some(at) = program.at("u_forward") {
        gl.uniform_3_f32(Some(at), forward.x as f32, forward.y as f32, forward.z as f32);
    }
}

unsafe fn set_projection(
    gl: &glow::Context,
    program: &Program,
    resident: &Resident,
    view: &View,
    section: Option<Plane>,
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

/// Which winding faces the eye on the card, for `GL_CULL_FACE` and for the
/// cap's winding count.
///
/// A triangle faces the eye when its corners run counter-clockwise seen from
/// there, which in the screen's right and up is counter-clockwise -- and the
/// rows flip up into the rasterizer's downward-counting rows, so on the card
/// it is clockwise. The basis is asked rather than assumed, so a camera that
/// ever mirrors the picture culls the right side still.
pub(super) fn front_face(view: &View) -> u32 {
    let (right, up) = view.basis();
    if right.cross(up).dot(-view.forward()) > 0.0 {
        glow::CW
    } else {
        glow::CCW
    }
}
