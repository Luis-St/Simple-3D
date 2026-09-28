//! The shader sources, and the vertex they take.

use crate::raster::{Rgba, Vertex};

mod csg;
mod ground;
mod mesh;
mod outline;
pub(crate) use csg::*;
pub(crate) use ground::*;
pub(crate) use mesh::*;
pub(crate) use outline::*;

/// One vertex as the shaders take it: screen position in pixels, depth key, straight-alpha
/// colour, body tag, and for an axis the segment index.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct GpuVertex {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) key: f32,
    pub(super) colour: [u8; 4],
    pub(super) tag: u32,
    pub(super) segment: u32,
}

impl GpuVertex {
    pub(super) fn new(v: Vertex, colour: Rgba, tag: u16, segment: u32) -> GpuVertex {
        GpuVertex { x: v.pos.x, y: v.pos.y, key: v.key, colour, tag: tag as u32, segment }
    }
}

pub(crate) const VERTEX_SOURCE: &str = r#"#version 330 core
layout(location = 0) in vec2 in_pos;
layout(location = 1) in float in_key;
layout(location = 2) in vec4 in_colour;
layout(location = 3) in uint in_tag;
layout(location = 4) in uint in_segment;

uniform vec2 u_viewport;
// Maps the rasterizer's depth key (larger is nearer) onto OpenGL's (smaller is nearer), so
// `GL_LESS` matches the software renderer's `key <= stored`. `x` is the offset, `y` the scale,
// with room at each end for line depth bias.
uniform vec2 u_depth;

out vec4 v_colour;
flat out uint v_tag;
flat out uint v_segment;
out float v_depth;

void main() {
    // The rasterizer counts rows down from the top, OpenGL up from the bottom, and egui takes the
    // texture's first row as the top. The two flips cancel, so row 0 goes to the bottom here.
    // Getting this wrong mirrors the viewport in a way that still looks plausible.
    vec2 ndc = vec2(
        (in_pos.x / u_viewport.x) * 2.0 - 1.0,
        (in_pos.y / u_viewport.y) * 2.0 - 1.0
    );
    float depth = clamp(u_depth.x - in_key * u_depth.y, -1.0, 1.0);
    gl_Position = vec4(ndc, depth, 1.0);
    v_colour = in_colour;
    v_tag = in_tag;
    v_segment = in_segment;
    v_depth = depth * 0.5 + 0.5;
}
"#;

pub(crate) const SOLID_SOURCE: &str = r#"#version 330 core
in vec4 v_colour;
flat in uint v_tag;

layout(location = 0) out vec4 out_colour;
layout(location = 1) out uint out_tag;

void main() {
    out_colour = v_colour;
    out_tag = v_tag;
}
"#;

/// A full-screen gradient, matching `Palette::background_at`.
pub(crate) const BACKGROUND_VERTEX: &str = r#"#version 330 core
out vec2 v_uv;
void main() {
    // A triangle covering the viewport, from the vertex index alone.
    vec2 p = vec2((gl_VertexID << 1) & 2, gl_VertexID & 2);
    v_uv = p;
    gl_Position = vec4(p * 2.0 - 1.0, 1.0, 1.0);
}
"#;

pub(crate) const BACKGROUND_FRAGMENT: &str = r#"#version 330 core
in vec2 v_uv;
uniform vec4 u_top;
uniform vec4 u_bottom;
layout(location = 0) out vec4 out_colour;
layout(location = 1) out uint out_tag;
void main() {
    // Row 0 (the palette's top colour) is at the bottom of the framebuffer, so `v_uv.y` 0 is the top.
    out_colour = mix(u_top, u_bottom, v_uv.y);
    out_tag = 0u;
}
"#;

/// Common prelude for resident mesh shaders: the projection as three affine rows and the step
/// from screen position and depth key to clip space.
///
/// The rows are computed on the CPU in double precision per mesh per frame, and positions are
/// stored relative to the mesh's centre, so the single-precision work here is on small numbers.
/// `u_model` is the linear part of a drag move (the rest is in the rows), and `u_hide` is the
/// vertex range of the dragged body, not drawn here (see `render::Live`).
const RESIDENT_COMMON: &str = r#"
uniform vec2 u_viewport;
uniform vec2 u_depth;
uniform vec4 u_row_x;
uniform vec4 u_row_y;
uniform vec4 u_row_key;
uniform mat3 u_model;
uniform uvec2 u_hide;

bool hidden(uint vertex) {
    return vertex >= u_hide.x && vertex < u_hide.y;
}

vec3 project(vec3 p) {
    return vec3(
        dot(u_row_x.xyz, p) + u_row_x.w,
        dot(u_row_y.xyz, p) + u_row_y.w,
        dot(u_row_key.xyz, p) + u_row_key.w
    );
}

vec4 place(vec2 screen, float key) {
    vec2 ndc = vec2((screen.x / u_viewport.x) * 2.0 - 1.0, (screen.y / u_viewport.y) * 2.0 - 1.0);
    return vec4(ndc, clamp(u_depth.x - key * u_depth.y, -1.0, 1.0), 1.0);
}
"#;

/// Mesh topology tables for geometry shaders: positions, triangle corners and vertex bodies, in
/// rows of `u_table_width` texels (see `resident.rs`).
const TABLES_COMMON: &str = r#"
uniform int u_table_width;
uniform sampler2D u_positions;
uniform usampler2D u_triangles;
uniform usampler2D u_bodies;

ivec2 cell(uint index) {
    return ivec2(int(index) % u_table_width, int(index) / u_table_width);
}

vec3 position(uint vertex) {
    return u_model * texelFetch(u_positions, cell(vertex), 0).xyz;
}
"#;

/// The sections, tested per pixel. Each cut removes what is past all of its walls (one for a
/// plane, five for a windowed box), and together they remove what any of them removes, which no
/// clip distance can express. `u_walls` holds every cut's walls as distances positive past the
/// wall; `u_cut_walls` says how many belong to each cut.
///
/// `u_within` limits the line round a cut on one box face to that face, within `u_cut_slack`.
const BOX_COMMON: &str = r#"
uniform int u_cut_count;
uniform int u_cut_walls[8];
uniform vec4 u_walls[40];
uniform int u_within_count;
uniform vec4 u_within[4];
uniform float u_cut_slack;

bool cut_away(vec3 p) {
    int base = 0;
    for (int c = 0; c < u_cut_count; c++) {
        bool inside = true;
        for (int w = 0; w < u_cut_walls[c]; w++) {
            inside = inside && dot(u_walls[base + w].xyz, p) + u_walls[base + w].w > 0.0;
        }
        if (inside) {
            return true;
        }
        base += u_cut_walls[c];
    }
    for (int w = 0; w < u_within_count; w++) {
        if (dot(u_within[w].xyz, p) + u_within[w].w < -u_cut_slack) {
            return true;
        }
    }
    return false;
}
"#;

fn resident(stage: &str) -> String {
    format!("#version 330 core\n{RESIDENT_COMMON}\n{stage}")
}
