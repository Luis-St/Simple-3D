//! The grid (on the ground or a workplane) and origin axes, drawn on the card.
//!
//! The CPU renderer builds these as primitives in `render.rs`; the GPU gets only the deciding
//! parameters and computes every pixel itself.

use super::*;
use crate::render::{
    axis_layout, axis_material_live, effective_grid_spacing, grid_levels_on, grid_radius, grid_radius_on, Request,
    AXIS_BIAS, GRID_BIAS, WORKPLANE_BIAS,
};
use crate::view::View;
use eframe::glow::{self, HasContext};
use simple3d_geom::Vec3;

/// What one frame's grid and axes are drawn with.
pub(super) struct Ground {
    grid: Option<GridDraw>,
    axes: Vec<AxisDraw>,
    /// Per axis, the material spans along it: start, end and body tag.
    spans: [Vec<[f32; 4]>; 3],
}

struct GridDraw {
    /// The coarse level's snapped centre, which the quad is placed around.
    origin: Vec3,
    /// The plane's two directions, which the quad spans (issue 125).
    u: Vec3,
    v: Vec3,
    /// Whether it is a workplane's, drawn on the model rather than under it, and its depth bias.
    on_model: bool,
    bias: f32,
    /// How far the quad reaches each way.
    reach: f64,
    fine: f64,
    coarse: f64,
    strength: f64,
    half: [f64; 2],
    major: [f64; 2],
    /// Coarse lines per major line.
    every: f64,
    /// How heavy the emphasised lines are, and whether every coarse line is one (`GridPlane::bold`).
    bold: f64,
    bold_all: bool,
    minor: [f32; 4],
    major_colour: [f32; 4],
    fade: f64,
}

struct AxisDraw {
    axis: usize,
    /// The arm's two ends in world space, each with its position along the axis.
    ends: [[f32; 4]; 2],
    start: f64,
    reach: f64,
    away: f64,
    colour: [f32; 4],
}

/// Compute the frame's grid and axes, and widen the depth range to take them in.
pub(super) fn prepare(request: &Request<'_>, passes: &mut Passes) -> Ground {
    let view = &request.view;
    let grid = &request.grid;
    let origin_rows = resident::rows_at(view, Vec3::ZERO);
    let key_of = |p: Vec3| {
        let k = origin_rows[2];
        k[0] * p.x as f32 + k[1] * p.y as f32 + k[2] * p.z as f32 + k[3]
    };

    let grid_draw = grid.visible.then(|| {
        let plane = grid.plane.unwrap_or_else(|| crate::render::GridPlane::ground(grid.spacing));
        let (fine, coarse, strength) = grid_levels_on(view, &plane);
        let radius = grid_radius_on(view, &plane);
        let level_half = |spacing: f64| spacing * ((radius / spacing).ceil()).clamp(1.0, 400.0);
        let half = [level_half(fine), level_half(coarse)];
        let (s, t) = plane.local(view.camera().target);
        let centre = ((s / coarse).round() * coarse, (t / coarse).round() * coarse);
        let origin = plane.at(centre.0, centre.1);
        let reach = half[0].max(half[1]) + coarse;
        let on_model = grid.plane.is_some();
        let bias = if on_model { WORKPLANE_BIAS } else { GRID_BIAS };
        for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            let key = key_of(origin + plane.u * (x * reach) + plane.v * (y * reach));
            passes.saw(key);
            passes.saw(key + bias * key.abs());
        }
        let half_diagonal = ((view.size.x as f64).hypot(view.size.y as f64) / 2.0).max(1.0);
        let (bold, bold_all) = plane.bold(fine, coarse, strength);
        GridDraw {
            origin,
            u: plane.u,
            v: plane.v,
            on_model,
            bias,
            reach,
            fine,
            coarse,
            strength,
            half,
            major: [
                (centre.0 / coarse).round().rem_euclid(plane.every()),
                (centre.1 / coarse).round().rem_euclid(plane.every()),
            ],
            every: plane.every(),
            bold,
            bold_all,
            minor: as_float(request.palette.grid),
            major_colour: as_float(request.palette.grid_major),
            fade: half_diagonal * 1.08,
        }
    });

    let material = axis_material_live(&request.items, grid, &request.section, &request.live);
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
        let start = centre.get(axis);
        for sign in [-1.0, 1.0] {
            let end = centre + direction * (length * sign);
            let point = |p: Vec3| [p.x as f32, p.y as f32, p.z as f32, p.get(axis) as f32];
            for p in [centre, end] {
                passes.saw(key_of(p) + AXIS_BIAS);
            }
            axes.push(AxisDraw {
                axis,
                ends: [point(centre), point(end)],
                start,
                reach,
                away: view.forward().get(axis),
                colour: as_float(colours[axis]),
            });
        }
    }
    Ground { grid: grid_draw, axes, spans }
}

impl Ground {
    /// Whether the grid is a workplane's, drawn over the finished model rather than under it.
    pub(super) fn grid_on_model(&self) -> bool {
        self.grid.as_ref().is_some_and(|grid| grid.on_model)
    }
}

impl Gpu {
    /// The grid: blended, writing neither depth nor tag, under the model or, a workplane's, on it.
    /// Expects the model pass's framebuffer with only colour drawn to.
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
            set4(gl, program, name, &row);
        }
        let floats = [
            ("u_bias", grid.bias),
            ("u_fine", grid.fine as f32),
            ("u_coarse", grid.coarse as f32),
            ("u_strength", grid.strength as f32),
            ("u_fade", grid.fade as f32),
            ("u_every", grid.every as f32),
            ("u_bold", grid.bold as f32),
            ("u_bold_all", if grid.bold_all { 1.0 } else { 0.0 }),
        ];
        for (name, value) in floats {
            set_f32(gl, program, name, value);
        }
        let pairs = [
            ("u_viewport", viewport),
            ("u_depth", depth),
            ("u_half", [grid.half[0] as f32, grid.half[1] as f32]),
            ("u_major", [grid.major[0] as f32, grid.major[1] as f32]),
        ];
        for (name, value) in pairs {
            set2(gl, program, name, [value[0], value[1]]);
        }
        for (name, value) in [("u_minor_colour", grid.minor), ("u_major_colour", grid.major_colour)] {
            set4(gl, program, name, &value);
        }
        for (name, axis) in [("u_span_u", grid.u), ("u_span_v", grid.v)] {
            set4(gl, program, name, &[axis.x as f32, axis.y as f32, axis.z as f32, 0.0]);
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

    /// The axes, in the overlay pass where depth and tag buffers are readable.
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
            set4(gl, program, name, &row);
        }
        for (name, value) in [("u_viewport", viewport), ("u_depth", depth)] {
            set2(gl, program, name, [value[0], value[1]]);
        }
        set_f32(gl, program, "u_bias", AXIS_BIAS);
        for (unit, (name, texture)) in
            [("u_depth_tex", target.depth), ("u_tag_tex", target.tags), ("u_spans", self.buffer.seen)]
                .into_iter()
                .enumerate()
        {
            gl.active_texture(glow::TEXTURE0 + unit as u32);
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            set_i32(gl, program, name, unit as i32);
        }
        gl.bind_vertex_array(Some(self.ground.array));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.ground.vertices));
        for arm in &ground.axes {
            set_i32(gl, program, "u_row", arm.axis as i32);
            set_i32(gl, program, "u_count", ground.spans[arm.axis].len() as i32);
            for (name, value) in [("u_start", arm.start), ("u_reach", arm.reach), ("u_away", arm.away)] {
                set_f32(gl, program, name, value as f32);
            }
            set4(gl, program, "u_colour", &arm.colour);
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes_of(&arm.ends), glow::STREAM_DRAW);
            gl.vertex_attrib_pointer_f32(0, 4, glow::FLOAT, false, 16, 0);
            gl.draw_arrays(glow::LINES, 0, 2);
        }
        gl.active_texture(glow::TEXTURE0);
        gl.bind_vertex_array(Some(self.buffer.array));
    }
}

fn bytes_of<T: Copy>(values: &[T]) -> &[u8] {
    // Only ever handed plain numbers and arrays of them, which have no padding.
    unsafe { std::slice::from_raw_parts(values.as_ptr() as *const u8, std::mem::size_of_val(values)) }
}
