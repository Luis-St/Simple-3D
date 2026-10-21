//! The model's edges a dragged cutter quiets, on the card (issue 129): the GPU's counterpart of
//! `render::Quiet`. The cutters' triangles are uploaded once when the dragged cutters change, and
//! each frame only their placements are set; the line geometry stage then drops every feature line
//! lying on one (`QUIET_COMMON`). Before, only the per-pixel boolean quieted its creases, so a drag
//! the card could not preview showed the cut as the evaluation's black lines.

use super::resident::{bytes_of, table, Format, Resident};
use super::*;
use crate::render::{Live, Renderable};
use eframe::glow::HasContext;
use simple3d_core::xform::Xform;
use simple3d_geom::aabb::box_corner;
use simple3d_geom::Vec3;

/// As many placements as the shader takes (`QUIET_MAX`); a drag carrying more cutters quiets the first.
const QUIET_MAX: usize = 8;

/// The dragged cutters' triangles on the card, three texels each relative to `origin`.
pub(super) struct QuietTable {
    /// The renderables uploaded, in order, so a new drag rebuilds and a moving one does not.
    ids: Vec<u64>,
    texture: glow::Texture,
    origin: Vec3,
    /// Each renderable's first triangle, triangle count and box relative to `origin`.
    parts: Vec<([i32; 2], Vec3, Vec3)>,
    /// This frame's placements: which part, turned and shifted into the table's frame.
    placements: Vec<(usize, Xform)>,
}

impl Gpu {
    /// Upload the cutters `live.quiet` names when they change and place them for this frame; drop
    /// the table when nothing is dragged.
    pub(super) unsafe fn keep_quiet(&mut self, gl: &glow::Context, live: &Live<'_>) -> Result<(), String> {
        let mut ids: Vec<u64> = Vec::new();
        let mut shapes: Vec<&Renderable> = Vec::new();
        for (renderable, _) in &live.quiet {
            if !ids.contains(&renderable.id) {
                ids.push(renderable.id);
                shapes.push(renderable);
            }
        }
        if self.quiet.as_ref().is_some_and(|table| table.ids != ids) || ids.is_empty() {
            if let Some(table) = self.quiet.take() {
                gl.delete_texture(table.texture);
            }
        }
        if ids.is_empty() {
            return Ok(());
        }
        if self.quiet.is_none() {
            self.quiet = Some(upload(gl, ids, &shapes, self.table_width)?);
        }
        let table = self.quiet.as_mut().expect("uploaded above");
        table.placements = live
            .quiet
            .iter()
            .take(QUIET_MAX)
            .filter_map(|(renderable, moved)| table.ids.iter().position(|&id| id == renderable.id).map(|p| (p, *moved)))
            .collect();
        Ok(())
    }

    /// Set the quiet test for one line draw of `resident` at `placing`: on for the model's edges
    /// while cutters are dragged, off otherwise.
    pub(super) unsafe fn set_quiet(
        &self,
        gl: &glow::Context,
        program: &Program,
        resident: &Resident,
        placing: &resident::Placing,
        wanted: bool,
    ) {
        let table = self.quiet.as_ref().filter(|table| wanted && !table.placements.is_empty());
        set_i32(gl, program, "u_quiet", table.is_some() as i32);
        let Some(table) = table else { return };
        gl.active_texture(glow::TEXTURE2);
        gl.bind_texture(glow::TEXTURE_2D, Some(table.texture));
        gl.active_texture(glow::TEXTURE0);
        set_i32(gl, program, "u_quiet_triangles", 2);
        set_i32(gl, program, "u_quiet_width", self.table_width as i32);
        set_i32(gl, program, "u_quiet_count", table.placements.len() as i32);
        let (mut ranges, mut turns, mut shifts) = (Vec::new(), Vec::new(), Vec::new());
        let mut lo = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut hi = -lo;
        for (part, moved) in &table.placements {
            let (range, part_lo, part_hi) = table.parts[*part];
            ranges.extend(range);
            // `moved` acts on world points; in the table's frame it shifts by where it takes the origin.
            turns.extend(moved.m.as_flattened().iter().map(|&value| value as f32));
            let shift = moved.point(table.origin) - table.origin;
            shifts.extend([shift.x as f32, shift.y as f32, shift.z as f32]);
            for corner in 0..8 {
                let at = moved.vector(box_corner(part_lo, part_hi, corner)) + shift;
                lo = lo.min(at);
                hi = hi.max(at);
            }
        }
        if let Some(at) = program.at("u_quiet_range[0]") {
            gl.uniform_2_i32_slice(Some(at), &ranges);
        }
        if let Some(at) = program.at("u_quiet_turn[0]") {
            gl.uniform_matrix_3_f32_slice(Some(at), true, &turns);
        }
        if let Some(at) = program.at("u_quiet_shift[0]") {
            gl.uniform_3_f32_slice(Some(at), &shifts);
        }
        let offset = resident.placed_origin(placing) - table.origin;
        let vec3 = |v: Vec3| [v.x as f32, v.y as f32, v.z as f32];
        for (name, value) in [("u_quiet_offset", offset), ("u_quiet_lo", lo), ("u_quiet_hi", hi)] {
            if let Some(at) = program.at(name) {
                let [x, y, z] = vec3(value);
                gl.uniform_3_f32(Some(at), x, y, z);
            }
        }
        // As near as `Quiet::new` asks, plus single-precision error of the numbers in play.
        let near = 1e-4 * (hi - lo).length().max(1.0) + (offset.length() + (hi - lo).length()) * 4e-6;
        set_f32(gl, program, "u_quiet_near", near as f32);
    }
}

/// Lay the shapes' triangles out round the middle of all of them.
unsafe fn upload(
    gl: &glow::Context,
    ids: Vec<u64>,
    shapes: &[&Renderable],
    width: usize,
) -> Result<QuietTable, String> {
    let bounds: Vec<(Vec3, Vec3)> =
        shapes.iter().map(|shape| shape.mesh.bounds().unwrap_or((Vec3::ZERO, Vec3::ZERO))).collect();
    let (lo, hi) = bounds.iter().fold(
        (
            Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY),
            Vec3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY),
        ),
        |(lo, hi), &(a, b)| (lo.min(a), hi.max(b)),
    );
    let origin = (lo + hi) * 0.5;
    let mut texels: Vec<[f32; 4]> = Vec::new();
    let mut parts = Vec::new();
    for (shape, (part_lo, part_hi)) in shapes.iter().zip(bounds) {
        let start = (texels.len() / 3) as i32;
        for triangle in &shape.mesh.indices {
            for corner in triangle {
                let p = shape.mesh.positions[*corner as usize] - origin;
                texels.push([p.x as f32, p.y as f32, p.z as f32, 0.0]);
            }
        }
        parts.push(([start, shape.mesh.indices.len() as i32], part_lo - origin, part_hi - origin));
    }
    let texture = table(gl, width, Format::Vec4, bytes_of(&texels), texels.len())?;
    Ok(QuietTable { ids, texture, origin, parts, placements: Vec::new() })
}
