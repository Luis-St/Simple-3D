//! The grid, the origin axes and a tool's preview, drawn on the card.
//!
//! None of them comes off a mesh, and the CPU renderer works each out as
//! primitives in `render.rs`. The GPU is handed only what decides them -- the
//! grid's levels and reach, where each axis's arms run and the stretches of
//! material along it, the preview's loops once -- and finds every pixel of
//! them itself, so the frame costs the CPU nothing per line.

use super::*;
use crate::render::{
    axis_layout, axis_material_live, effective_grid_spacing, grid_levels, grid_radius, Renderable, Request, GRID_BIAS,
};
use crate::view::View;
use eframe::glow::{self, HasContext};
use simple3d_geom::Vec3;

/// What one frame's grid and axes are drawn with.
pub(super) struct Ground {
    grid: Option<GridDraw>,
    axes: Vec<AxisDraw>,
    /// Per axis, the stretches of material along it: where each starts and
    /// ends and the tag of the body it is in.
    spans: [Vec<[f32; 4]>; 3],
}

struct GridDraw {
    /// The coarse level's snapped centre, which the quad is placed around.
    origin: Vec3,
    /// How far the quad reaches each way from it.
    reach: f64,
    fine: f64,
    coarse: f64,
    strength: f64,
    half: [f64; 2],
    major: [f64; 2],
    minor: [f32; 4],
    major_colour: [f32; 4],
    fade: f64,
}

struct AxisDraw {
    axis: usize,
    /// The arm's two ends in the world, each with where it is along the axis.
    ends: [[f32; 4]; 2],
    start: f64,
    reach: f64,
    away: f64,
    colour: [f32; 4],
}

/// Work out the frame's grid and axes, and widen the depth range to take them
/// in.
pub(super) fn prepare(request: &Request<'_>, passes: &mut Passes) -> Ground {
    let view = &request.view;
    let grid = &request.grid;
    let origin_rows = resident::rows_at(view, Vec3::ZERO);
    let key_of = |p: Vec3| {
        let k = origin_rows[2];
        k[0] * p.x as f32 + k[1] * p.y as f32 + k[2] * p.z as f32 + k[3]
    };

    let grid_draw = grid.visible.then(|| {
        let (fine, coarse, strength) = grid_levels(view, grid.spacing);
        let radius = grid_radius(view);
        let level_half = |spacing: f64| spacing * ((radius / spacing).ceil()).clamp(1.0, 400.0);
        let half = [level_half(fine), level_half(coarse)];
        let target = view.camera().target;
        let origin = Vec3::new((target.x / coarse).round() * coarse, (target.y / coarse).round() * coarse, 0.0);
        let reach = half[0].max(half[1]) + coarse;
        for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            let key = key_of(origin + Vec3::new(x * reach, y * reach, 0.0));
            passes.saw(key);
            passes.saw(key + GRID_BIAS * key.abs());
        }
        let half_diagonal = ((view.size.x as f64).hypot(view.size.y as f64) / 2.0).max(1.0);
        GridDraw {
            origin,
            reach,
            fine,
            coarse,
            strength,
            half,
            major: [(origin.x / coarse).round().rem_euclid(10.0), (origin.y / coarse).round().rem_euclid(10.0)],
            minor: as_float(request.palette.grid),
            major_colour: as_float(request.palette.grid_major),
            fade: half_diagonal * 1.08,
        }
    });

    let material = axis_material_live(&request.items, grid, request.section, &request.live);
    let spans: [Vec<[f32; 4]>; 3] = std::array::from_fn(|axis| {
        material.through[axis].iter().map(|&(lo, hi, tag)| [lo as f32, hi as f32, tag as f32, 0.0]).collect()
    });
    let spacing = effective_grid_spacing(view, grid.spacing);
    let radius = grid_radius(view);
    let colours = [request.palette.axis_x, request.palette.axis_y, request.palette.axis_z];
    let mut axes = Vec::new();
    for axis in (0..3).filter(|&axis| grid.axes[axis]) {
        let (centre, length, reach) = axis_layout(view, grid, spacing, radius, material.reach, axis);
        let direction = crate::render::along(axis, 1.0);
        let start = crate::render::component(centre, axis);
        for sign in [-1.0, 1.0] {
            let end = centre + direction * (length * sign);
            let point = |p: Vec3| [p.x as f32, p.y as f32, p.z as f32, crate::render::component(p, axis) as f32];
            for p in [centre, end] {
                passes.saw(key_of(p) + AXIS_BIAS);
            }
            axes.push(AxisDraw {
                axis,
                ends: [point(centre), point(end)],
                start,
                reach,
                away: crate::render::component(view.forward(), axis),
                colour: as_float(colours[axis]),
            });
        }
    }
    Ground { grid: grid_draw, axes, spans }
}

impl Gpu {
    /// The grid: under the model, blended, and claiming neither depth nor a
    /// body. Expects the model pass's framebuffer, with colour alone drawn to.
    pub(super) unsafe fn draw_grid(
        &self,
        gl: &glow::Context,
        ground: &Ground,
        view: &View,
        viewport: [f32; 2],
        depth: [f32; 2],
    ) {
        let Some(grid) = &ground.grid else { return };
        let program = &self.grid;
        gl.use_program(Some(program.program));
        let rows = resident::rows_at(view, grid.origin);
        for (name, row) in ["u_row_x", "u_row_y", "u_row_key"].into_iter().zip(rows) {
            if let Some(at) = program.at(name) {
                gl.uniform_4_f32_slice(Some(at), &row);
            }
        }
        let floats = [
            ("u_bias", GRID_BIAS),
            ("u_fine", grid.fine as f32),
            ("u_coarse", grid.coarse as f32),
            ("u_strength", grid.strength as f32),
            ("u_fade", grid.fade as f32),
        ];
        for (name, value) in floats {
            if let Some(at) = program.at(name) {
                gl.uniform_1_f32(Some(at), value);
            }
        }
        let pairs = [
            ("u_viewport", viewport),
            ("u_depth", depth),
            ("u_half", [grid.half[0] as f32, grid.half[1] as f32]),
            ("u_major", [grid.major[0] as f32, grid.major[1] as f32]),
        ];
        for (name, value) in pairs {
            if let Some(at) = program.at(name) {
                gl.uniform_2_f32(Some(at), value[0], value[1]);
            }
        }
        for (name, value) in [("u_minor_colour", grid.minor), ("u_major_colour", grid.major_colour)] {
            if let Some(at) = program.at(name) {
                gl.uniform_4_f32_slice(Some(at), &value);
            }
        }
        let r = grid.reach as f32;
        let quad: [[f32; 2]; 6] = [[-r, -r], [r, -r], [r, r], [-r, -r], [r, r], [-r, r]];
        gl.bind_vertex_array(Some(self.ground.array));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.ground.vertices));
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes_of(&quad), glow::STREAM_DRAW);
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 8, 0);
        gl.draw_arrays(glow::TRIANGLES, 0, 6);
        gl.bind_vertex_array(Some(self.buffer.array));
    }

    /// The axes, in the overlay pass, where the depth and tag buffers are
    /// readable rather than attached.
    pub(super) unsafe fn draw_axes(
        &self,
        gl: &glow::Context,
        ground: &Ground,
        view: &View,
        viewport: [f32; 2],
        depth: [f32; 2],
    ) {
        if ground.axes.is_empty() {
            return;
        }
        let target = self.target.as_ref().expect("drawn after the resize");
        // The spans, one row per axis.
        let width = ground.spans.iter().map(Vec::len).max().unwrap_or(0).max(1);
        let mut table = vec![[0.0_f32; 4]; width * 3];
        for (axis, spans) in ground.spans.iter().enumerate() {
            table[axis * width..axis * width + spans.len()].copy_from_slice(spans);
        }
        gl.bind_texture(glow::TEXTURE_2D, Some(self.buffer.seen));
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA32F as i32,
            width as i32,
            3,
            0,
            glow::RGBA,
            glow::FLOAT,
            glow::PixelUnpackData::Slice(Some(bytes_of(&table))),
        );

        let program = &self.axis;
        gl.use_program(Some(program.program));
        let rows = resident::rows_at(view, Vec3::ZERO);
        for (name, row) in ["u_row_x", "u_row_y", "u_row_key"].into_iter().zip(rows) {
            if let Some(at) = program.at(name) {
                gl.uniform_4_f32_slice(Some(at), &row);
            }
        }
        for (name, value) in [("u_viewport", viewport), ("u_depth", depth)] {
            if let Some(at) = program.at(name) {
                gl.uniform_2_f32(Some(at), value[0], value[1]);
            }
        }
        if let Some(at) = program.at("u_bias") {
            gl.uniform_1_f32(Some(at), AXIS_BIAS);
        }
        for (unit, (name, texture)) in
            [("u_depth_tex", target.depth), ("u_tag_tex", target.tags), ("u_spans", self.buffer.seen)]
                .into_iter()
                .enumerate()
        {
            gl.active_texture(glow::TEXTURE0 + unit as u32);
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            if let Some(at) = program.at(name) {
                gl.uniform_1_i32(Some(at), unit as i32);
            }
        }
        gl.bind_vertex_array(Some(self.ground.array));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.ground.vertices));
        for arm in &ground.axes {
            if let Some(at) = program.at("u_row") {
                gl.uniform_1_i32(Some(at), arm.axis as i32);
            }
            if let Some(at) = program.at("u_count") {
                gl.uniform_1_i32(Some(at), ground.spans[arm.axis].len() as i32);
            }
            for (name, value) in [("u_start", arm.start), ("u_reach", arm.reach), ("u_away", arm.away)] {
                if let Some(at) = program.at(name) {
                    gl.uniform_1_f32(Some(at), value as f32);
                }
            }
            if let Some(at) = program.at("u_colour") {
                gl.uniform_4_f32_slice(Some(at), &arm.colour);
            }
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes_of(&arm.ends), glow::STREAM_DRAW);
            gl.vertex_attrib_pointer_f32(0, 4, glow::FLOAT, false, 16, 0);
            gl.draw_arrays(glow::LINES, 0, 2);
        }
        gl.active_texture(glow::TEXTURE0);
        gl.bind_vertex_array(Some(self.buffer.array));
    }

    /// A tool's preview loops as lines the card keeps, made again only when
    /// the loops change.
    pub(super) fn refresh_preview(&mut self, request: &Request<'_>) {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for loop_ in &request.preview {
            loop_.len().hash(&mut hasher);
            for p in loop_ {
                [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()].hash(&mut hasher);
            }
        }
        let key = hasher.finish();
        if request.preview.is_empty() {
            self.preview = None;
            return;
        }
        if self.preview.as_ref().is_some_and(|(held, _)| *held == key) {
            return;
        }
        let mut positions = Vec::new();
        let mut edges = Vec::new();
        for loop_ in &request.preview {
            let first = positions.len() as u32;
            positions.extend_from_slice(loop_);
            let count = loop_.len() as u32;
            edges.extend((0..count).map(|index| [first + index, first + (index + 1) % count]));
        }
        self.preview = Some((key, Renderable::lines(positions, edges)));
    }
}

fn bytes_of<T: Copy>(values: &[T]) -> &[u8] {
    // Only ever handed plain numbers and arrays of them, which have no padding.
    unsafe { std::slice::from_raw_parts(values.as_ptr() as *const u8, std::mem::size_of_val(values)) }
}
