//! Uploading resident meshes to the card and freeing them.

use super::*;

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
            edge_faces: None,
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
            if !item.edges.is_empty() && item.edge_faces.len() == item.edges.len() {
                let corners = far_corners(item, &positions);
                self.edge_faces = Some(table(gl, width, Format::Vec4, bytes_of(&corners), corners.len())?);
            }
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

    pub(in crate::gpu) unsafe fn destroy(&self, gl: &glow::Context) {
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
        for texture in [self.paint, self.edge_faces].into_iter().flatten() {
            gl.delete_texture(texture);
        }
    }
}

/// Two texels per feature edge: each neighbouring triangle's corner off the edge, stored like the
/// positions, and in `w` the sign that turns `cross(b - a, corner - a)` outward for edge `[a, b]`.
fn far_corners(item: &Renderable, positions: &[[f32; 3]]) -> Vec<[f32; 4]> {
    let mut corners = Vec::with_capacity(item.edges.len() * 2);
    for (edge, faces) in item.edges.iter().zip(&item.edge_faces) {
        for &face in faces {
            let tri = item.mesh.indices[face as usize];
            let at = (0..3).find(|&k| tri[k] != edge[0] && tri[k] != edge[1]).unwrap_or(0);
            // Wound a -> b inside the triangle, the cross product already points out.
            let forwards = tri[(at + 1) % 3] == edge[0];
            let [x, y, z] = positions[tri[at] as usize];
            corners.push([x, y, z, if forwards { 1.0 } else { -1.0 }]);
        }
    }
    corners
}

/// How a table's texels are laid out.
#[derive(Clone, Copy)]
enum Format {
    Vec3,
    Vec4,
    UVec3,
    U16,
    U32,
}

impl Format {
    /// Internal format, pixel format, component type and bytes per texel.
    fn gl(self) -> (u32, u32, u32, usize) {
        match self {
            Format::Vec3 => (glow::RGB32F, glow::RGB, glow::FLOAT, 12),
            Format::Vec4 => (glow::RGBA32F, glow::RGBA, glow::FLOAT, 16),
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
    clamp_texture(gl, glow::NEAREST, glow::NEAREST);
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
    pub(in crate::gpu) unsafe fn keep_resident(
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
    pub(in crate::gpu) unsafe fn keep_half_space(&mut self, gl: &glow::Context) -> Result<(), String> {
        let Some((_, shapes)) = &self.csg_half else { return Ok(()) };
        for (half, _) in shapes {
            let resident = self.resident.entry(half.id).or_insert_with(|| Resident::new(half));
            resident.ensure(gl, half, Needs { faces: true, ..Needs::default() }, self.table_width)?;
        }
        Ok(())
    }
}
