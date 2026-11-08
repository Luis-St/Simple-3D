//! Sending a renderable edited in place to the card piece by piece (the brush, issue 127): only the
//! changed vertices' positions and the changed triangles' corner normals, with
//! `glBufferSubData` and `glTexSubImage2D`, instead of uploading the mesh again for every dab.
//!
//! The feature edges' slope corners (`edge_faces`) are left as they were: they only tune a line's
//! depth bias, and the evaluation after the stroke uploads a fresh mesh anyway.

use super::*;
use crate::render::Edits;

impl Resident {
    /// Bring the card's copy from `edits.since` to `edits.revision`. Parts not uploaded yet are
    /// left alone; they go up from the renderable as it now is when first needed.
    pub(super) unsafe fn edit(
        &mut self,
        gl: &glow::Context,
        item: &Renderable,
        edits: &Edits,
        width: usize,
    ) -> Result<(), String> {
        let origin = self.origin;
        let stored = |index: usize| {
            let p = item.mesh.positions[index] - origin;
            [p.x as f32, p.y as f32, p.z as f32]
        };
        for range in &edits.vertices {
            let positions: Vec<[f32; 3]> = range.clone().map(|v| stored(v as usize)).collect();
            // The depth range is sized from the extent, so a vertex pushed out of it grows it.
            for p in &positions {
                let p = Vec3::new(p[0] as f64, p[1] as f64, p[2] as f64);
                (self.lo, self.hi) = (self.lo.min(p), self.hi.max(p));
            }
            if let Some(vertices) = &self.vertices {
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertices.positions));
                gl.buffer_sub_data_u8_slice(glow::ARRAY_BUFFER, range.start as i32 * 12, bytes_of(&positions));
                gl.bind_buffer(glow::ARRAY_BUFFER, None);
            }
            if let Some(tables) = &self.tables {
                let total = item.mesh.positions.len();
                retable(gl, tables.positions, width, Format::Vec3, total, range, bytes_of(&positions));
            }
        }
        if let Some(corners) = self.corners {
            let total = item.corner_normals.len() * 3;
            for range in &edits.triangles {
                let normals: Vec<[f32; 3]> = item.corner_normals[range.start as usize..range.end as usize]
                    .iter()
                    .flat_map(|corners| corners.map(|n| [n.x as f32, n.y as f32, n.z as f32]))
                    .collect();
                let texels = range.start * 3..range.end * 3;
                retable(gl, corners, width, Format::Vec3, total, &texels, bytes_of(&normals));
            }
        }
        self.revision = edits.revision;
        Ok(())
    }
}
