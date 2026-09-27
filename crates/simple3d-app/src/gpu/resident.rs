//! Meshes the GPU renderer keeps on the card between frames.
//!
//! Uploaded once and projected per frame from the camera alone; everything camera-independent
//! (faces, edges, outlines, crossings, caps) is derived on the card. Positions are stored in
//! single precision relative to the mesh's centre, and each part is uploaded only when first needed.

use super::*;
use crate::raster::Rgba;
use crate::render::{
    mark_colours, shade, tag_bases, to_vertex, Live, Palette, Renderable, Request, Style, EDGE_BIAS, EDGE_ON,
    MARK_BIAS, PREVIEW_BIAS, SELECTION_BIAS, SELECTION_CREASE,
};
use crate::snap::MARK_AXIS;
use crate::view::View;
use eframe::glow::{self, HasContext};
use simple3d_core::config::DisplayMode;
use simple3d_core::xform::Xform;
use simple3d_geom::section::Plane;
use simple3d_geom::Vec3;

/// A vertex array and the element or vertex buffer it draws from.
pub(super) struct Batch {
    pub(super) array: glow::VertexArray,
    buffer: glow::Buffer,
    pub(super) count: i32,
}

/// The mesh's vertices: position and body per welded vertex.
struct Vertices {
    positions: glow::Buffer,
    bodies: glow::Buffer,
}

/// The mesh laid out in textures, for stages that look up neighbouring triangles (the outline).
struct Tables {
    positions: glow::Texture,
    triangles: glow::Texture,
    bodies: glow::Texture,
}

pub(crate) struct Resident {
    /// Mesh centre the positions are stored relative to, keeping single-precision values small.
    origin: Vec3,
    /// Mesh extent relative to `origin`, used to size the depth range.
    lo: Vec3,
    hi: Vec3,
    vertices: Option<Vertices>,
    pub(super) faces: Option<Batch>,
    /// One colour tag per triangle, when any triangle is painted.
    pub(super) paint: Option<glow::Texture>,
    pub(super) edges: Option<Batch>,
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

/// Where a resident is drawn this frame: moved by a drag, with a vertex range hidden.
#[derive(Clone, Copy)]
pub(super) struct Placing {
    xform: Xform,
    hide: [u32; 2],
}

impl Placing {
    pub(super) const NONE: Placing = Placing { xform: Xform::IDENTITY, hide: [0, 0] };

    pub(super) fn moved(xform: Option<Xform>) -> Placing {
        Placing { xform: xform.unwrap_or(Xform::IDENTITY), hide: [0, 0] }
    }

    fn of(live: &Live, id: u64) -> Placing {
        Placing {
            xform: live.placed(id).copied().unwrap_or(Xform::IDENTITY),
            hide: live.hidden(id).map_or([0, 0], |range| [range.start, range.end]),
        }
    }
}

/// What one frame asks of the resident meshes.
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
    /// Tool preview lines drawn over the model, depth-tested but not written.
    pub(super) overlays: Vec<LineDraw>,
    /// Shapes a boolean preview is drawn from (`csg.rs`).
    pub(super) csg: Vec<u64>,
    /// Edge colour of a boolean preview, when the display mode draws lines.
    pub(super) csg_edges: Option<Rgba>,
}

pub(super) struct FaceDraw {
    id: u64,
    placing: Placing,
    mode: i32,
    base: Rgba,
    tag_base: u16,
}

pub(super) struct LineDraw {
    id: u64,
    placing: Placing,
    colour: Rgba,
    bias: f32,
    tag_base: Option<u16>,
}

impl LineDraw {
    /// A tool's preview loops, biased towards the eye like `push_preview`.
    pub(super) fn preview(id: u64, colour: Rgba) -> LineDraw {
        LineDraw { id, placing: Placing::NONE, colour, bias: PREVIEW_BIAS, tag_base: None }
    }
}

pub(super) struct OutlineDraw {
    id: u64,
    placing: Placing,
    colour: Rgba,
    tag_base: u16,
    /// Every crease rather than only those facing the eye: wireframe.
    all_creases: bool,
}

pub(super) struct CrossingDraw {
    id: u64,
    placing: Placing,
    /// Each plane, and the colour its crossing is drawn in.
    planes: Vec<(Plane, Rgba)>,
    /// Sections that cut the crossing. A cut's own edge is cut only by the others,
    /// since its own plane test would fray it.
    cuts: Vec<Plane>,
    /// The other walls of the box whose face the line lies on (`BOX_COMMON`'s `u_within`).
    within: Vec<Plane>,
}

pub(super) struct CapDraw {
    id: u64,
    placing: Placing,
    pub(super) plane: Plane,
    /// The other walls of a windowed section's box; empty for an unbounded plane.
    pub(super) bounds: Vec<Plane>,
    /// Other sections, whose cuts are taken out of this cap.
    pub(super) others: Vec<Plane>,
    pub(super) colour: Rgba,
    /// The plane's polygon within the mesh's box, projected.
    pub(super) fill: Vec<GpuVertex>,
}

const SOLID: i32 = 0;
const GHOST: i32 = 1;
const GLOW: i32 = 2;

/// Which resident draws a frame needs, mirroring `render::prepare_with`.
pub(super) fn plan(request: &Request<'_>) -> Plan {
    let palette: &Palette = &request.palette;
    let mut plan = Plan::default();
    if request.mode == DisplayMode::ShadedWithEdges {
        plan.csg_edges = Some(palette.edge);
    }
    for (item, tag_base) in request.items.iter().zip(tag_bases(&request.items)) {
        let id = item.renderable.id;
        let placing = Placing::of(&request.live, id);
        match item.style {
            Style::Solid => {
                let solid = FaceDraw { id, placing, mode: SOLID, base: opaque(palette.solid), tag_base };
                match request.mode {
                    DisplayMode::Wireframe => {
                        plan.lines.push(LineDraw { id, placing, colour: palette.wire, bias: 0.0, tag_base: None })
                    }
                    DisplayMode::Shaded => plan.solids.push(solid),
                    DisplayMode::ShadedWithEdges => {
                        plan.solids.push(solid);
                        plan.lines.push(LineDraw {
                            id,
                            placing,
                            colour: palette.edge,
                            bias: EDGE_BIAS,
                            tag_base: Some(tag_base),
                        });
                    }
                }
                // One cap per face the cut opens. A box side facing away from the camera is not filled,
                // which the winding count also needs; its line is still drawn as the opening's near rim.
                let forward = request.view.forward();
                for (cut, section) in request.section.iter().enumerate() {
                    let others: Vec<Plane> =
                        request.section.iter().enumerate().filter(|&(o, _)| o != cut).map(|(_, p)| *p).collect();
                    for (index, face) in simple3d_geom::section::faces(section).into_iter().enumerate() {
                        let plane = face.plane;
                        let facing = index == 0 || plane.normal.dot(forward) < 0.0;
                        if request.mode != DisplayMode::Wireframe && facing {
                            let colour = shade(palette.cut, plane.normal, forward, 255);
                            plan.caps.push(CapDraw {
                                id,
                                placing,
                                plane,
                                bounds: face.bounds.clone(),
                                others: others.clone(),
                                colour,
                                fill: Vec::new(),
                            });
                        }
                        if request.mode != DisplayMode::Shaded {
                            let colour = match request.mode {
                                DisplayMode::Wireframe => palette.wire,
                                _ => palette.edge,
                            };
                            plan.crossings.push(CrossingDraw {
                                id,
                                placing,
                                planes: vec![(plane, colour)],
                                cuts: others.clone(),
                                within: face.bounds,
                            });
                        }
                    }
                }
            }
            Style::Ghost => plan.ghosts.push(FaceDraw { id, placing, mode: GHOST, base: palette.ghost, tag_base: 0 }),
            Style::Glow | Style::Selected => {
                if item.style == Style::Glow {
                    plan.glows.push(FaceDraw { id, placing, mode: GLOW, base: palette.glow, tag_base: 0 });
                }
                // Without adjacency only creases can outline a body (`push_selection`'s fallback).
                if item.renderable.outline.is_empty() {
                    plan.lines.push(LineDraw {
                        id,
                        placing,
                        colour: palette.selected,
                        bias: SELECTION_BIAS,
                        tag_base: Some(tag_base),
                    });
                } else {
                    plan.outlines.push(OutlineDraw {
                        id,
                        placing,
                        colour: palette.selected,
                        tag_base,
                        all_creases: request.mode == DisplayMode::Wireframe,
                    });
                }
            }
        }
    }
    let planes = mark_planes(request);
    if !planes.is_empty() {
        for item in request.items.iter().filter(|item| item.style == Style::Solid) {
            let id = item.renderable.id;
            let placing = Placing::of(&request.live, id);
            plan.crossings.push(CrossingDraw {
                id,
                placing,
                planes: planes.clone(),
                cuts: request.section.clone(),
                within: Vec::new(),
            });
        }
    }
    plan
}

/// The principal planes whose marks this frame draws, with colours.
pub(super) fn mark_planes(request: &Request<'_>) -> Vec<(Plane, Rgba)> {
    if !request.grid.plane_marks || request.mode == DisplayMode::Wireframe {
        return Vec::new();
    }
    let colours = mark_colours(&request.palette);
    (0..3)
        .filter(|&axis| request.grid.axes[MARK_AXIS[axis]])
        .map(|axis| (Plane::on_axis(axis, 0.0, false), colours[axis]))
        .collect()
}

impl Plan {
    fn needs(&self) -> std::collections::HashMap<u64, Needs> {
        let mut needs: std::collections::HashMap<u64, Needs> = std::collections::HashMap::new();
        let faces = self.solids.iter().chain(&self.ghosts).chain(&self.glows).map(|draw| draw.id);
        let faces = faces.chain(self.crossings.iter().map(|draw| draw.id)).chain(self.caps.iter().map(|draw| draw.id));
        let faces = faces.chain(self.csg.iter().copied());
        for id in faces {
            needs.entry(id).or_default().faces = true;
        }
        if self.csg_edges.is_some() {
            for &id in &self.csg {
                needs.entry(id).or_default().edges = true;
            }
        }
        for draw in self.lines.iter().chain(&self.overlays) {
            needs.entry(draw.id).or_default().edges = true;
        }
        for draw in &self.outlines {
            needs.entry(draw.id).or_default().outline = true;
        }
        needs
    }
}

/// A solid is drawn opaque regardless of palette alpha, as in `push_shaded`.
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
            // Junctions are never on the outline (`BorderEdge::junction`), so they are dropped here.
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

    /// The projection rows (screen x, screen y, depth key) for this mesh, folded with the
    /// storage offset in double precision. When moved, they apply after `u_model`.
    fn rows(&self, view: &View, placing: &Placing) -> [[f32; 4]; 3] {
        rows_at(view, placing.xform.point(self.origin))
    }

    /// A plane as the shaders test it, positive where `Plane::depth` is.
    pub(super) fn plane(&self, plane: &Plane, placing: &Placing) -> [f32; 4] {
        let n = plane.normal;
        let origin = placing.xform.point(self.origin);
        [n.x as f32, n.y as f32, n.z as f32, (n.dot(origin) - plane.offset) as f32]
    }

    /// The section plane as a clip distance: positive where the model is kept.
    fn clip(&self, section: Option<Plane>, placing: &Placing) -> [f32; 4] {
        match section {
            Some(plane) => self.plane(&plane, placing).map(|value| -value),
            None => [0.0, 0.0, 0.0, 1.0],
        }
    }

    /// Every cut's walls for `BOX_COMMON`, and how many belong to each; cuts past the shader limit are dropped.
    fn cuts(&self, cuts: &[Plane], placing: &Placing) -> (Vec<i32>, Vec<[f32; 4]>) {
        let (mut counts, mut walls) = (Vec::new(), Vec::new());
        for cut in cuts.iter().take(simple3d_geom::section::MAX_CUTS) {
            let own = cut.walls();
            counts.push(own.len() as i32);
            walls.extend(own.iter().map(|wall| self.plane(wall, placing)));
        }
        (counts, walls)
    }

    /// The largest stored coordinate, which single-precision error is relative to.
    fn extent(&self) -> f64 {
        let lo = self.lo.x.abs().max(self.lo.y.abs()).max(self.lo.z.abs());
        let hi = self.hi.x.abs().max(self.hi.y.abs()).max(self.hi.z.abs());
        lo.max(hi)
    }

    /// The mesh's world-space box, moved by `placing`.
    pub(super) fn world_box(&self, placing: &Placing) -> (Vec3, Vec3) {
        let mut lo = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut hi = -lo;
        for corner in 0..8 {
            let local = Vec3::new(
                if corner & 1 == 0 { self.lo.x } else { self.hi.x },
                if corner & 2 == 0 { self.lo.y } else { self.hi.y },
                if corner & 4 == 0 { self.lo.z } else { self.hi.z },
            );
            let world = placing.xform.point(self.origin + local);
            lo = lo.min(world);
            hi = hi.max(world);
        }
        (lo, hi)
    }

    /// The depth keys of this mesh's box corners under `view`.
    pub(super) fn keys(&self, view: &View, placing: &Placing) -> impl Iterator<Item = f32> {
        let [_, _, key] = self.rows(view, placing);
        let (lo, hi, xform) = (self.lo, self.hi, placing.xform);
        (0..8).map(move |corner| {
            let x = if corner & 1 == 0 { lo.x } else { hi.x };
            let y = if corner & 2 == 0 { lo.y } else { hi.y };
            let z = if corner & 4 == 0 { lo.z } else { hi.z };
            let p = xform.vector(Vec3::new(x, y, z));
            key[0] * p.x as f32 + key[1] * p.y as f32 + key[2] * p.z as f32 + key[3]
        })
    }

    /// The polygon `plane` leaves in this mesh's box, in order. The box is slightly enlarged so a
    /// cut lying exactly on a box face still meets it.
    pub(super) fn cap_polygon(&self, plane: &Plane, placing: &Placing) -> Vec<Vec3> {
        let margin = Vec3::new(1.0, 1.0, 1.0) * (self.extent() * 1e-3 + 1e-6);
        let (lo, hi) = (self.origin + self.lo - margin, self.origin + self.hi + margin);
        let corner = |index: usize| {
            placing.xform.point(Vec3::new(
                if index & 1 == 0 { lo.x } else { hi.x },
                if index & 2 == 0 { lo.y } else { hi.y },
                if index & 4 == 0 { lo.z } else { hi.z },
            ))
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

/// `count` texels as a texture `width` texels wide, filled row by row. The short last row is
/// uploaded separately so nothing needs padding.
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

unsafe fn buffer(gl: &glow::Context, target: u32, bytes: &[u8]) -> Result<glow::Buffer, String> {
    let buffer = gl.create_buffer()?;
    gl.bind_buffer(target, Some(buffer));
    gl.buffer_data_u8_slice(target, bytes, glow::STATIC_DRAW);
    Ok(buffer)
}

/// A vertex array over the mesh's vertices, drawing `indices`.
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
    /// Upload what `plan` draws and free what it no longer draws, so replaced meshes do not
    /// linger in video memory.
    pub(super) unsafe fn keep_resident(
        &mut self,
        gl: &glow::Context,
        request: &Request<'_>,
        plan: &Plan,
        extra: &[&Renderable],
    ) -> Result<(), String> {
        let drawn: Vec<&Renderable> =
            request.items.iter().map(|item| item.renderable).chain(extra.iter().copied()).collect();
        let wanted: std::collections::HashSet<u64> = drawn.iter().map(|renderable| renderable.id).collect();
        let stale: Vec<u64> = self.resident.keys().copied().filter(|id| !wanted.contains(id)).collect();
        for id in stale {
            if let Some(resident) = self.resident.remove(&id) {
                resident.destroy(gl);
            }
        }
        let needs = plan.needs();
        for renderable in drawn {
            let id = renderable.id;
            let resident = self.resident.entry(id).or_insert_with(|| Resident::new(renderable));
            let need = needs.get(&id).copied().unwrap_or_default();
            resident.ensure(gl, renderable, need, self.table_width)?;
        }
        Ok(())
    }

    /// Upload the section's half-space for a boolean preview; sized from the other shapes (`csg.rs`).
    pub(super) unsafe fn keep_half_space(&mut self, gl: &glow::Context) -> Result<(), String> {
        let Some((_, shapes)) = &self.csg_half else { return Ok(()) };
        for (half, _) in shapes {
            let resident = self.resident.entry(half.id).or_insert_with(|| Resident::new(half));
            resident.ensure(gl, half, Needs { faces: true, ..Needs::default() }, self.table_width)?;
        }
        Ok(())
    }

    /// Widen the depth range to take in a boolean preview's shapes.
    pub(super) fn see_csg(&self, request: &Request<'_>, passes: &mut Passes) {
        let Some(csg) = &request.live.csg else { return };
        let half = self.csg_half.iter().flat_map(|(_, shapes)| shapes.iter().map(|(half, _)| (half, None)));
        for (shape, moved) in csg.leaves.iter().map(|(leaf, moved)| (*leaf, *moved)).chain(half) {
            let Some(resident) = self.resident.get(&shape.id) else { continue };
            for key in resident.keys(&request.view, &Placing::moved(moved)) {
                passes.saw(key);
            }
        }
    }

    /// Compute each cap's fill polygon and widen the depth range to take it in.
    pub(super) fn place_caps(&self, view: &View, plan: &mut Plan, passes: &mut Passes) {
        for cap in &mut plan.caps {
            let Some(resident) = self.resident.get(&cap.id) else { continue };
            let polygon = simple3d_geom::section::within(&resident.cap_polygon(&cap.plane, &cap.placing), &cap.bounds);
            // Cut by the other sections here rather than per pixel: the fill is in screen space and
            // has no world position to test.
            for index in 1..polygon.len().saturating_sub(1) {
                let corners = [polygon[0], polygon[index], polygon[index + 1]];
                for piece in simple3d_geom::section::clip_by_all(&cap.others, corners).triangles() {
                    for &at in piece {
                        let vertex = to_vertex(view, view.to_view(at));
                        passes.saw(vertex.key);
                        cap.fill.push(GpuVertex::new(vertex, cap.colour, 0, 0));
                    }
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
    pub(super) unsafe fn draw_lines(
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

    /// Selected and glowing bodies' outlines.
    pub(super) unsafe fn draw_outlines(
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
            set_projection(gl, program, resident, view, section, &draw.placing);
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

    /// Plane marks and section cut edges on the resident meshes.
    pub(super) unsafe fn draw_crossings(
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
        if let Some(at) = program.at("u_bias") {
            gl.uniform_1_f32(Some(at), MARK_BIAS);
        }
        for draw in draws {
            let Some(resident) = self.resident.get(&draw.id) else { continue };
            let Some(faces) = &resident.faces else { continue };
            set_projection(gl, program, resident, view, &draw.cuts, &draw.placing);
            set_within(gl, program, resident, &draw.placing, &draw.within);
            let planes: Vec<[f32; 4]> =
                draw.planes.iter().map(|(plane, _)| resident.plane(plane, &draw.placing)).collect();
            let colours: Vec<[f32; 4]> = draw.planes.iter().map(|&(_, colour)| as_float(colour)).collect();
            set_i32(gl, program, "u_planes", planes.len() as i32);
            if let Some(at) = program.at("u_plane[0]") {
                gl.uniform_4_f32_slice(Some(at), planes.as_flattened());
            }
            if let Some(at) = program.at("u_plane_colour[0]") {
                gl.uniform_4_f32_slice(Some(at), colours.as_flattened());
            }
            // A few times the single-precision error of the largest number in play.
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

    /// A section's caps. The stencil counts how often the kept surface winds round each pixel
    /// (front faces up, back faces down), and the plane's box polygon is filled where the count is
    /// non-zero. Winding rather than parity, so overlapping bodies count as material.
    ///
    /// Expects the model pass's state, and restores it.
    #[allow(clippy::too_many_arguments)]
    pub(super) unsafe fn draw_caps(
        &self,
        gl: &glow::Context,
        draws: &[CapDraw],
        view: &View,
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
            gl.enable(glow::CLIP_DISTANCE1);
            set2(gl, program, "u_viewport", viewport);
            set2(gl, program, "u_depth", depth);
            // Counted against the cap's whole face only: the window box and other sections trim
            // the cap afterwards.
            set_projection(gl, program, resident, view, &[], &cap.placing);
            if let Some(at) = program.at("u_clip") {
                gl.uniform_4_f32_slice(Some(at), &resident.clip(Some(cap.plane), &cap.placing));
            }
            set_i32(gl, program, "u_mode", GHOST);
            set_i32(gl, program, "u_painted", 0);
            gl.bind_vertex_array(Some(faces.array));
            gl.draw_elements(glow::TRIANGLES, faces.count, glow::UNSIGNED_INT, 0);
            gl.disable(glow::CLIP_DISTANCE0);
            gl.disable(glow::CLIP_DISTANCE1);

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

    /// Widen the depth range to take in every resident mesh drawn, plus the largest line bias.
    pub(super) fn see_resident(&self, request: &Request<'_>, plan: &Plan, passes: &mut Passes) {
        let items =
            request.items.iter().map(|item| (item.renderable.id, Placing::of(&request.live, item.renderable.id)));
        let overlays = plan.overlays.iter().map(|draw| (draw.id, draw.placing));
        for (id, placing) in items.chain(overlays) {
            let Some(resident) = self.resident.get(&id) else { continue };
            for key in resident.keys(&request.view, &placing) {
                passes.saw(key);
                passes.saw(key + MARK_BIAS * key.abs());
            }
        }
    }
}

pub(super) unsafe fn set2(gl: &glow::Context, program: &Program, name: &str, value: [f32; 2]) {
    if let Some(at) = program.at(name) {
        gl.uniform_2_f32(Some(at), value[0], value[1]);
    }
}

pub(super) unsafe fn set_i32(gl: &glow::Context, program: &Program, name: &str, value: i32) {
    if let Some(at) = program.at(name) {
        gl.uniform_1_i32(Some(at), value);
    }
}

pub(super) unsafe fn set_forward(gl: &glow::Context, program: &Program, view: &View) {
    let forward = view.forward();
    if let Some(at) = program.at("u_forward") {
        gl.uniform_3_f32(Some(at), forward.x as f32, forward.y as f32, forward.z as f32);
    }
}

pub(super) unsafe fn set_projection(
    gl: &glow::Context,
    program: &Program,
    resident: &Resident,
    view: &View,
    section: &[Plane],
    placing: &Placing,
) {
    let rows = resident.rows(view, placing);
    for (name, row) in ["u_row_x", "u_row_y", "u_row_key"].into_iter().zip(rows) {
        if let Some(at) = program.at(name) {
            gl.uniform_4_f32_slice(Some(at), &row);
        }
    }
    // Sections are tested per pixel (`BOX_COMMON`); the clip distance is only for the cap count pass.
    if let Some(at) = program.at("u_clip") {
        gl.uniform_4_f32_slice(Some(at), &resident.clip(None, placing));
    }
    if let Some(at) = program.at("u_cut_count") {
        let (counts, walls) = resident.cuts(section, placing);
        gl.uniform_1_i32(Some(at), counts.len() as i32);
        if let Some(at) = program.at("u_cut_walls[0]") {
            if !counts.is_empty() {
                gl.uniform_1_i32_slice(Some(at), &counts);
            }
        }
        if let Some(at) = program.at("u_walls[0]") {
            if !walls.is_empty() {
                gl.uniform_4_f32_slice(Some(at), walls.as_flattened());
            }
        }
        set_within(gl, program, resident, placing, &[]);
    }
    if let Some(at) = program.at("u_model") {
        // Row-major, as `Xform` stores its matrix.
        let m: Vec<f32> = placing.xform.m.as_flattened().iter().map(|&value| value as f32).collect();
        gl.uniform_matrix_3_f32_slice(Some(at), true, &m);
    }
    if let Some(at) = program.at("u_hide") {
        gl.uniform_2_u32(Some(at), placing.hide[0], placing.hide[1]);
    }
}

/// The walls a line must stay inside, for `BOX_COMMON`'s `u_within`.
pub(super) unsafe fn set_within(
    gl: &glow::Context,
    program: &Program,
    resident: &Resident,
    placing: &Placing,
    within: &[Plane],
) {
    let Some(count_at) = program.at("u_within_count") else { return };
    let walls: Vec<[f32; 4]> = within.iter().take(4).map(|wall| resident.plane(wall, placing)).collect();
    gl.uniform_1_i32(Some(count_at), walls.len() as i32);
    if walls.is_empty() {
        return;
    }
    if let Some(at) = program.at("u_within[0]") {
        gl.uniform_4_f32_slice(Some(at), walls.as_flattened());
    }
    if let Some(at) = program.at("u_cut_slack") {
        // A few times the single-precision error of the numbers in play, as for `u_on_plane`.
        let offset = walls.iter().fold(0.0_f64, |most, wall| most.max(wall[3].abs() as f64));
        gl.uniform_1_f32(Some(at), ((resident.extent() + offset) * 4e-6 + 1e-6) as f32);
    }
}

/// Which winding faces the eye on the card, for culling and the cap's winding count.
/// Counter-clockwise on screen becomes clockwise after the row flip; the basis is checked
/// rather than assumed so a mirrored camera still culls correctly.
pub(super) fn front_face(view: &View) -> u32 {
    let (right, up) = view.basis();
    if right.cross(up).dot(-view.forward()) > 0.0 {
        glow::CW
    } else {
        glow::CCW
    }
}

/// The projection rows for positions stored relative to `origin`, in double precision.
pub(super) fn rows_at(view: &View, origin: Vec3) -> [[f32; 4]; 3] {
    let (right, up) = view.basis();
    let forward = view.forward();
    let s = view.pixels_per_mm();
    let d = origin - view.eye();
    let row = |v: Vec3, c: f64| [v.x as f32, v.y as f32, v.z as f32, c as f32];
    [
        row(right * s, view.centre.x as f64 + d.dot(right) * s),
        row(-(up * s), view.centre.y as f64 - d.dot(up) * s),
        row(-forward, -d.dot(forward)),
    ]
}
