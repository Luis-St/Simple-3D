//! The shader sources, and the vertex they take.

use crate::raster::{Rgba, Vertex};

/// One vertex as the shaders want it: screen position in pixels, depth key,
/// straight-alpha colour, the body tag, and -- for an axis -- which of its
/// segments this is, so the shader can look up that segment's own rule.
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
// Maps the rasterizer's depth key -- larger is nearer -- onto OpenGL's, where
// smaller is nearer, so `GL_LESS` decides exactly what `key <= stored` decides
// in the software renderer. `x` is the offset and `y` the scale: the nearest
// key in the frame lands just inside the front of the buffer and the furthest
// just inside the back, with room at each end for the depth bias a line gets.
uniform vec2 u_depth;

out vec4 v_colour;
flat out uint v_tag;
flat out uint v_segment;
out float v_depth;

void main() {
    // The rasterizer counts rows downwards from the top; OpenGL counts them
    // upwards from the bottom -- and egui, painting this texture, takes its
    // first row to be the top of the rect. The two flips cancel, so row 0 goes
    // to the *bottom* of the framebuffer here and comes out at the top on
    // screen. Getting this the other way round mirrors the whole viewport,
    // which reads as a plausible picture until it is measured against the
    // software renderer's.
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

/// A full-screen gradient, the same one `Palette::background_at` lays down.
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
    // Row 0 of the frame -- the top, and the palette's first colour -- is at
    // the bottom of the framebuffer, so `v_uv.y` of 0 is the palette's top.
    out_colour = mix(u_top, u_bottom, v_uv.y);
    out_tag = 0u;
}
"#;

/// What every shader drawing a resident mesh starts with: the projection, as
/// three rows applied to a stored position, and the step from a screen
/// position and depth key to OpenGL's clip space.
///
/// The projection is orthographic and so affine: each screen coordinate and
/// the depth key are a dot product with the vertex plus a constant, and those
/// rows are worked out on the CPU in double precision once per mesh per frame.
/// Positions are stored relative to the mesh's own centre, so the
/// single-precision arithmetic left here is on small numbers. The screen
/// coordinates are the rasterizer's own, rows counted down from the top -- see
/// `VERTEX_SOURCE` for why that is the right way up.
///
/// A body being dragged is drawn moved (see `render::Live`): `u_model` is the
/// linear part of the move, put through every stored position before
/// anything else is done with it, and the rest of the move is in the rows.
/// `u_hide` is the stretch of vertices of a mesh that is not drawn at all --
/// the dragged body's part of the scene -- and is empty otherwise.
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

/// The per-vertex and per-triangle tables a geometry shader reads a mesh's
/// topology from: positions, the corners of each triangle and the body of each
/// vertex, laid out in rows of `u_table_width` texels (see `resident.rs`).
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

fn resident(stage: &str) -> String {
    format!("#version 330 core\n{RESIDENT_COMMON}\n{stage}")
}

/// A resident mesh's faces, projected on the card and culled by the pipeline.
///
/// The arithmetic is `push_shaded`'s, `push_ghost`'s and `push_glow`'s,
/// moved here so that an orbit hands the card a camera rather than a million
/// projected triangles. Nothing is stored per triangle but its corners: the
/// face's normal is the one its own pixels span (see `FACE_FRAGMENT`), and a
/// solid's back faces are dropped by `GL_CULL_FACE`, which under a parallel
/// projection asks exactly what `push_shaded` asks -- whether the triangle
/// turns towards the eye.
pub(crate) fn face_vertex() -> String {
    resident(
        r#"
layout(location = 0) in vec3 in_pos;
layout(location = 1) in uint in_body;

uniform uint u_tag_base;
// The section plane as a distance that is positive on the side that is kept.
uniform vec4 u_clip;

out vec3 v_pos;
flat out uint v_tag;
// The boolean preview draws the same faces in several programs and compares
// the depths they land at, which only holds if every program puts a vertex in
// exactly the same place (`gpu/csg.rs`).
invariant gl_Position;

void main() {
    vec3 pos = u_model * in_pos;
    vec3 screen = project(pos);
    gl_Position = place(screen.xy, screen.z);
    gl_ClipDistance[0] = dot(u_clip.xyz, pos) + u_clip.w;
    // A triangle with any corner hidden goes, and a hidden part shares no
    // corner with anything else -- see `Renderable::parts`.
    gl_ClipDistance[1] = hidden(uint(gl_VertexID)) ? -1.0 : 1.0;
    v_pos = pos;
    v_tag = min(u_tag_base + in_body + 1u, 65535u);
}
"#,
    )
}

/// A face's colour. Its normal is the plane its own pixels span -- the cross
/// product of the position's screen derivatives, which is the same for every
/// pixel of a flat triangle -- so no normal has to be stored or uploaded for
/// it. A painted face's colour comes out of the paint table by the triangle's
/// index.
pub(crate) const FACE_FRAGMENT: &str = r#"#version 330 core
in vec3 v_pos;
flat in uint v_tag;

uniform vec3 u_forward;
// The colour a face is drawn in when its own paint does not say otherwise, in
// bytes: the palette's solid, ghost or glow.
uniform vec4 u_base;
// 0 a solid, 1 a ghost, 2 a glow.
uniform int u_mode;
// Whether the mesh has any paint, and the table saying which triangle has
// which: the mesh's own colour tags, one texel per triangle.
uniform int u_painted;
uniform usampler2D u_paint;
uniform int u_table_width;

layout(location = 0) out vec4 out_colour;
layout(location = 1) out uint out_tag;

void main() {
    vec3 normal = cross(dFdx(v_pos), dFdy(v_pos));
    float length_ = length(normal);
    float facing = length_ > 0.0 ? abs(dot(normal / length_, u_forward)) : 0.0;

    // A painted face keeps its paint; alpha is the base's either way, exactly
    // as `triangle_base` hands it to `shade`.
    vec4 base = u_base;
    if (u_mode == 0 && u_painted == 1) {
        ivec2 at = ivec2(gl_PrimitiveID % u_table_width, gl_PrimitiveID / u_table_width);
        uint tag = texelFetch(u_paint, at, 0).r;
        if ((tag & 0xFF000000u) != 0u) {
            base = vec4(float((tag >> 16) & 255u), float((tag >> 8) & 255u), float(tag & 255u), u_base.a);
        }
    }
    if (u_mode == 2) {
        out_colour = base / 255.0;
    } else {
        // `shade`: a headlight plus a constant fill, truncated to a byte the
        // way the CPU's `as u8` does.
        float factor = 0.34 + 0.66 * facing;
        out_colour = vec4(floor(min(base.rgb * factor, vec3(255.0))), base.a) / 255.0;
    }
    out_tag = v_tag;
}
"#;

/// A resident mesh's feature edges, read straight out of the mesh's vertex
/// buffer by the edge list. The geometry stage sees both ends at once, so the
/// line gets exactly the bias `line_step` gives it: a fraction of the mean of
/// its two ends' keys.
pub(crate) fn line_vertex() -> String {
    resident(
        r#"
layout(location = 0) in vec3 in_pos;
layout(location = 1) in uint in_body;

// Whether the line claims its body's tag, or 0 like the wireframe.
uniform int u_tagged;
uniform uint u_tag_base;
uniform vec4 u_clip;

out vec3 g_screen;
out float g_clip;
flat out uint g_tag;
flat out int g_hidden;

void main() {
    vec3 pos = u_model * in_pos;
    g_screen = project(pos);
    g_clip = dot(u_clip.xyz, pos) + u_clip.w;
    g_tag = u_tagged == 1 ? min(u_tag_base + in_body + 1u, 65535u) : 0u;
    g_hidden = hidden(uint(gl_VertexID)) ? 1 : 0;
    gl_Position = vec4(0.0, 0.0, 0.0, 1.0);
}
"#,
    )
}

pub(crate) fn line_geometry() -> String {
    resident(
        r#"
layout(lines) in;
layout(line_strip, max_vertices = 2) out;

in vec3 g_screen[];
in float g_clip[];
flat in uint g_tag[];
flat in int g_hidden[];

uniform vec4 u_colour;
uniform float u_bias;

out vec4 v_colour;
flat out uint v_tag;

void main() {
    if (g_hidden[0] == 1 || g_hidden[1] == 1) {
        return;
    }
    float bias = u_bias * (abs(g_screen[0].z) + abs(g_screen[1].z)) * 0.5;
    for (int i = 0; i < 2; i++) {
        gl_Position = place(g_screen[i].xy, g_screen[i].z + bias);
        gl_ClipDistance[0] = g_clip[i];
        v_colour = u_colour;
        v_tag = g_tag[0];
        EmitVertex();
    }
    EndPrimitive();
}
"#,
    )
}

/// A selected body's outline, found on the card: `push_selection`, one edge
/// of the surface per point.
///
/// Each point is an edge -- its two ends and the two triangles along it -- and
/// the geometry stage looks the triangles up in the mesh's tables, asks which
/// of them face the eye, and draws the edge when the surface turns away across
/// it, or when it is a corner on the side facing the camera. A silhouette edge
/// is drawn a second time one pixel out from the shape, which is what makes it
/// a line rather than a row of dots; `push_selection` says why.
pub(crate) const OUTLINE_VERTEX: &str = r#"#version 330 core
layout(location = 0) in uvec4 in_edge;

flat out uvec4 g_edge;

void main() {
    g_edge = in_edge;
    gl_Position = vec4(0.0, 0.0, 0.0, 1.0);
}
"#;

pub(crate) fn outline_geometry() -> String {
    resident(&format!(
        "{TABLES_COMMON}\n{}",
        r#"
layout(points) in;
layout(line_strip, max_vertices = 4) out;

flat in uvec4 g_edge[];

uniform vec3 u_forward;
// `EDGE_ON`, and the cosine of `SELECTION_CREASE`.
uniform float u_edge_on;
uniform float u_crease;
// Wireframe takes the creases facing away as well.
uniform int u_all_creases;
uniform vec4 u_colour;
uniform float u_bias;
uniform uint u_tag_base;
uniform vec4 u_clip;

out vec4 v_colour;
flat out uint v_tag;

void face(uint index, out vec3 normal, out vec3 centre) {
    uvec3 corners = texelFetch(u_triangles, cell(index), 0).xyz;
    vec3 a = position(corners.x);
    vec3 b = position(corners.y);
    vec3 c = position(corners.z);
    vec3 n = cross(b - a, c - a);
    float length_ = length(n);
    normal = length_ > 0.0 ? n / length_ : vec3(0.0);
    centre = (a + b + c) / 3.0;
}

void line(vec3 a, vec3 b, float clip_a, float clip_b, vec2 shift, float bias, uint tag) {
    gl_Position = place(a.xy + shift, a.z + bias);
    gl_ClipDistance[0] = clip_a;
    v_colour = u_colour;
    v_tag = tag;
    EmitVertex();
    gl_Position = place(b.xy + shift, b.z + bias);
    gl_ClipDistance[0] = clip_b;
    v_colour = u_colour;
    v_tag = tag;
    EmitVertex();
    EndPrimitive();
}

void main() {
    uvec4 edge = g_edge[0];
    if (hidden(edge.x)) {
        return;
    }
    vec3 near_normal, near_centre, far_normal, far_centre;
    face(edge.z, near_normal, near_centre);
    face(edge.w, far_normal, far_centre);
    bool near_faces = dot(near_normal, u_forward) < -u_edge_on;
    bool far_faces = dot(far_normal, u_forward) < -u_edge_on;

    vec3 a = position(edge.x);
    vec3 b = position(edge.y);
    vec3 sa = project(a);
    vec3 sb = project(b);
    float bias = u_bias * (abs(sa.z) + abs(sb.z)) * 0.5;
    float clip_a = dot(u_clip.xyz, a) + u_clip.w;
    float clip_b = dot(u_clip.xyz, b) + u_clip.w;
    uint tag = min(u_tag_base + texelFetch(u_bodies, cell(edge.x), 0).r + 1u, 65535u);

    if (edge.z == edge.w || near_faces != far_faces) {
        line(sa, sb, clip_a, clip_b, vec2(0.0), bias, tag);
        // One pixel out of the shape: perpendicular to the edge on screen and
        // away from the centre of the face on the shape's own side of it.
        vec2 along = sb.xy - sa.xy;
        vec2 normal = vec2(-along.y, along.x);
        if (length(normal) < 1e-6) {
            return;
        }
        normal = normalize(normal);
        vec3 inside = near_faces ? near_centre : far_centre;
        vec2 inward = project(inside).xy - sa.xy;
        vec2 out_ = dot(normal, inward) > 0.0 ? -normal : normal;
        line(sa, sb, clip_a, clip_b, out_, bias, tag);
    } else if ((u_all_creases == 1 || near_faces) && dot(near_normal, far_normal) < u_crease) {
        line(sa, sb, clip_a, clip_b, vec2(0.0), bias, tag);
    }
}
"#
    ))
}

/// Where planes cross a resident mesh's surface, one segment per triangle per
/// plane: the principal planes' marks, and the edge a section leaves round the
/// cut. `snap::plane_crossing` and `section::loops` ask the same question of
/// every triangle on the CPU; asked here, it costs the CPU nothing at all.
///
/// A corner counts as lying on a plane within `u_on_plane`, a distance a few
/// times what single precision loses on this mesh's coordinates: the CPU
/// tests world positions for exact zeros, which the stored positions -- moved
/// to the mesh's centre and rounded to single precision -- no longer hit.
pub(crate) fn crossing_vertex() -> String {
    resident(
        r#"
layout(location = 0) in vec3 in_pos;
layout(location = 1) in uint in_body;

out vec3 g_pos;
flat out int g_hidden;

void main() {
    g_pos = u_model * in_pos;
    g_hidden = hidden(uint(gl_VertexID)) ? 1 : 0;
    gl_Position = vec4(0.0, 0.0, 0.0, 1.0);
}
"#,
    )
}

pub(crate) fn crossing_geometry() -> String {
    resident(
        r#"
layout(triangles) in;
layout(line_strip, max_vertices = 6) out;

in vec3 g_pos[];
flat in int g_hidden[];

// Up to three planes, each a distance from the stored position, and the
// colour its crossing is drawn in.
uniform int u_planes;
uniform vec4 u_plane[3];
uniform vec4 u_plane_colour[3];
uniform float u_on_plane;
uniform float u_bias;
uniform vec4 u_clip;

out vec4 v_colour;
flat out uint v_tag;

void main() {
    if (g_hidden[0] == 1 || g_hidden[1] == 1 || g_hidden[2] == 1) {
        return;
    }
    for (int k = 0; k < u_planes; k++) {
        float d[3];
        for (int i = 0; i < 3; i++) {
            d[i] = dot(u_plane[k].xyz, g_pos[i]) + u_plane[k].w;
            if (abs(d[i]) <= u_on_plane) {
                d[i] = 0.0;
            }
        }
        if ((d[0] > 0.0 && d[1] > 0.0 && d[2] > 0.0) || (d[0] < 0.0 && d[1] < 0.0 && d[2] < 0.0)) {
            continue;
        }
        // A triangle lying in the plane has no crossing of its own: its edges
        // are its neighbours' crossings.
        if (d[0] == 0.0 && d[1] == 0.0 && d[2] == 0.0) {
            continue;
        }
        vec3 hits[4];
        int count = 0;
        for (int i = 0; i < 3; i++) {
            int j = (i + 1) % 3;
            if (d[i] == 0.0) {
                hits[count++] = g_pos[i];
            }
            if ((d[i] < 0.0 && d[j] > 0.0) || (d[i] > 0.0 && d[j] < 0.0)) {
                hits[count++] = mix(g_pos[i], g_pos[j], d[i] / (d[i] - d[j]));
            }
        }
        if (count < 2) {
            continue;
        }
        vec3 a = project(hits[0]);
        vec3 b = project(hits[1]);
        float bias = u_bias * (abs(a.z) + abs(b.z)) * 0.5;
        gl_Position = place(a.xy, a.z + bias);
        gl_ClipDistance[0] = dot(u_clip.xyz, hits[0]) + u_clip.w;
        v_colour = u_plane_colour[k];
        v_tag = 0u;
        EmitVertex();
        gl_Position = place(b.xy, b.z + bias);
        gl_ClipDistance[0] = dot(u_clip.xyz, hits[1]) + u_clip.w;
        v_colour = u_plane_colour[k];
        v_tag = 0u;
        EmitVertex();
        EndPrimitive();
    }
}
"#,
    )
}

/// The ground grid, worked out per pixel over one quad on the ground: what
/// `push_grid` lays down as a few hundred lines, each cut into fading steps.
///
/// A pixel is on a grid line when the line is within half a pixel of it,
/// measured across the line on screen -- which is what a one-pixel line
/// rasterized along it covers. The two levels are the ones `grid_levels`
/// picks, the finer faded in by its strength and the coarser drawn over it
/// with every tenth line in the major colour; and every pixel fades with its
/// distance from the middle of the frame, as each step of a CPU grid line
/// does. The quad is positioned relative to the coarse level's snapped centre,
/// which is a multiple of both spacings, so the lines are at multiples of the
/// spacing in its own coordinates.
pub(crate) fn grid_vertex() -> String {
    resident(
        r#"
layout(location = 0) in vec2 in_uv;

uniform float u_bias;

out vec2 v_uv;

void main() {
    vec3 screen = project(vec3(in_uv, 0.0));
    gl_Position = place(screen.xy, screen.z + u_bias * abs(screen.z));
    v_uv = in_uv;
}
"#,
    )
}

pub(crate) const GRID_FRAGMENT: &str = r#"#version 330 core
in vec2 v_uv;

uniform vec2 u_viewport;
// The two spacings, the finer's strength, and how far each level's lines run
// from the centre.
uniform float u_fine;
uniform float u_coarse;
uniform float u_strength;
uniform vec2 u_half;
// Which coarse line through the centre each way is, counted in tens, so the
// majors fall on the world's own multiples of ten.
uniform vec2 u_major;
uniform vec4 u_minor_colour;
uniform vec4 u_major_colour;
// The distance from the middle of the frame, in pixels, at which the grid has
// faded out.
uniform float u_fade;

layout(location = 0) out vec4 out_colour;

// How many pixels from the nearest line at `spacing` the pixel is, across it.
float pixels_off(float w, float spacing) {
    float off = abs(w - spacing * round(w / spacing));
    return off / max(length(vec2(dFdx(w), dFdy(w))), 1e-12);
}

bool major(float w, float spacing, float centre) {
    return mod(round(w / spacing) + centre, 10.0) == 0.0;
}

void main() {
    float distance_ = length(gl_FragCoord.xy - u_viewport * 0.5);
    float fade = 1.0 - pow(min(distance_ / u_fade, 1.0), 2.0);
    if (fade <= 0.03) {
        discard;
    }
    vec4 colour = vec4(0.0);
    float reach = max(abs(v_uv.x), abs(v_uv.y));
    if (u_strength > 0.03 && reach <= u_half.x) {
        if (pixels_off(v_uv.x, u_fine) <= 0.5 || pixels_off(v_uv.y, u_fine) <= 0.5) {
            colour = vec4(u_minor_colour.rgb, u_minor_colour.a * u_strength);
        }
    }
    if (reach <= u_half.y) {
        bool along_x = pixels_off(v_uv.x, u_coarse) <= 0.5;
        bool along_y = pixels_off(v_uv.y, u_coarse) <= 0.5;
        if (along_x || along_y) {
            bool is_major = (along_x && major(v_uv.x, u_coarse, u_major.x)) || (along_y && major(v_uv.y, u_coarse, u_major.y));
            vec4 line = is_major ? u_major_colour : u_minor_colour;
            // Over the finer line, as the CPU draws it after.
            float alpha = line.a + colour.a * (1.0 - line.a);
            vec3 rgb = alpha > 0.0 ? (line.rgb * line.a + colour.rgb * colour.a * (1.0 - line.a)) / alpha : line.rgb;
            colour = vec4(rgb, alpha);
        }
    }
    if (colour.a <= 0.0) {
        discard;
    }
    out_colour = vec4(colour.rgb, colour.a * fade);
}
"#;

/// An arm of an origin axis, and the rule that is not just a depth test --
/// `push_axis_line` and `Frame::line_through`, asked per pixel.
///
/// Each vertex carries where it is along its axis, so every pixel knows where
/// it is too: it fades with the distance from the arm's centre, is left out
/// wherever the axis runs through material, and where it loses the depth test
/// is drawn anyway if the body that won the pixel is one the axis is arriving
/// at. The spans of material come in a table, one row per axis: where each
/// runs along the axis, and the tag of the body it is in.
pub(crate) fn axis_vertex() -> String {
    resident(
        r#"
layout(location = 0) in vec4 in_point;

uniform float u_bias;

out float v_along;
out float v_depth;

void main() {
    vec3 screen = project(in_point.xyz);
    gl_Position = place(screen.xy, screen.z + u_bias);
    v_along = in_point.w;
    v_depth = gl_Position.z * 0.5 + 0.5;
}
"#,
    )
}

pub(crate) const AXIS_FRAGMENT: &str = r#"#version 330 core
in float v_along;
in float v_depth;

uniform sampler2D u_depth_tex;
uniform usampler2D u_tag_tex;
uniform sampler2D u_spans;
uniform int u_row;
uniform int u_count;
// Where along the axis the arm starts, and the distance its fade is measured
// against.
uniform float u_start;
uniform float u_reach;
// How the axis points relative to the view, which decides which side of a
// body is the approach to it.
uniform float u_away;
uniform vec4 u_colour;

out vec4 out_colour;

void main() {
    float fade = 1.0 - pow(min(abs(v_along - u_start) / (u_reach * 0.8), 1.0), 2.0);
    if (fade <= 0.03) {
        discard;
    }
    for (int i = 0; i < u_count; i++) {
        vec4 span = texelFetch(u_spans, ivec2(i, u_row), 0);
        if (v_along > span.x && v_along < span.y) {
            discard;
        }
    }
    ivec2 at = ivec2(gl_FragCoord.xy);
    if (v_depth >= texelFetch(u_depth_tex, at, 0).r) {
        uint owner = texelFetch(u_tag_tex, at, 0).r;
        bool arriving = false;
        for (int i = 0; i < u_count; i++) {
            vec4 span = texelFetch(u_spans, ivec2(i, u_row), 0);
            if (uint(span.z) == owner) {
                arriving = arriving || (u_away > 1e-9 ? v_along <= span.x : (u_away < -1e-9 ? v_along >= span.y : true));
            }
        }
        if (!arriving) {
            discard;
        }
    }
    out_colour = vec4(u_colour.rgb, u_colour.a * fade);
}
"#;

/// The boolean preview's layer pass: every face of every shape, keeping at each
/// pixel the nearest one behind the layer before -- one layer of the shapes'
/// surfaces, peeled off the front of what is left. What is kept is the face's
/// colour, worked out as `FACE_FRAGMENT` works out a solid's, and which shape
/// it belongs to. See `gpu/csg.rs`.
pub(crate) const CSG_PEEL_FRAGMENT: &str = r#"#version 330 core
in vec3 v_pos;

uniform sampler2D u_previous;
uniform int u_first;
// The pixels an earlier layer has found the surface at, and the depth of the
// rest of the scene: a layer behind it can never be seen.
uniform sampler2D u_done;
uniform sampler2D u_scene_depth;
uniform uint u_leaf;
uniform vec3 u_forward;
uniform vec4 u_base;
uniform int u_painted;
uniform usampler2D u_paint;
uniform int u_table_width;

layout(location = 0) out vec4 out_colour;
layout(location = 1) out uint out_leaf;

void main() {
    ivec2 pixel = ivec2(gl_FragCoord.xy);
    if (texelFetch(u_done, pixel, 0).r > 0.5 || gl_FragCoord.z >= texelFetch(u_scene_depth, pixel, 0).r) {
        discard;
    }
    if (u_first == 0 && gl_FragCoord.z <= texelFetch(u_previous, pixel, 0).r) {
        discard;
    }
    vec3 normal = cross(dFdx(v_pos), dFdy(v_pos));
    float length_ = length(normal);
    float facing = length_ > 0.0 ? abs(dot(normal / length_, u_forward)) : 0.0;
    vec4 base = u_base;
    if (u_painted == 1) {
        ivec2 at = ivec2(gl_PrimitiveID % u_table_width, gl_PrimitiveID / u_table_width);
        uint tag = texelFetch(u_paint, at, 0).r;
        if ((tag & 0xFF000000u) != 0u) {
            base = vec4(float((tag >> 16) & 255u), float((tag >> 8) & 255u), float(tag & 255u), u_base.a);
        }
    }
    float factor = 0.34 + 0.66 * facing;
    out_colour = vec4(floor(min(base.rgb * factor, vec3(255.0))), 255.0) / 255.0;
    out_leaf = u_leaf;
}
"#;

/// The boolean preview's counting pass: one shape's faces in front of the
/// layer, each adding one to that shape's own channel. How many times a ray
/// from the eye crosses a closed surface before a point says whether the
/// point is inside it: an odd count is inside.
pub(crate) const CSG_COUNT_FRAGMENT: &str = r#"#version 330 core
layout(location = 0) out vec4 out_0;
layout(location = 1) out vec4 out_1;
layout(location = 2) out vec4 out_2;
layout(location = 3) out vec4 out_3;
layout(location = 4) out vec4 out_4;
layout(location = 5) out vec4 out_5;
layout(location = 6) out vec4 out_6;
layout(location = 7) out vec4 out_7;

void main() {
    vec4 one = vec4(1.0 / 255.0);
    out_0 = one;
    out_1 = one;
    out_2 = one;
    out_3 = one;
    out_4 = one;
    out_5 = one;
    out_6 = one;
    out_7 = one;
}
"#;

/// The boolean preview's resolving pass, over the whole frame: where the
/// layer's point is on the result's surface -- the expression says one thing
/// just in front of it and the other just behind, which is its own shape's
/// count with one more crossing -- the point is drawn, at the layer's depth
/// and in the layer's colour, and the pixel is marked done so no layer behind
/// it is drawn there.
pub(crate) const CSG_RESOLVE_FRAGMENT: &str = r#"#version 330 core
uniform sampler2D u_layer;
uniform usampler2D u_leaf;
uniform sampler2D u_colour;
uniform sampler2D u_count_0;
uniform sampler2D u_count_1;
uniform sampler2D u_count_2;
uniform sampler2D u_count_3;
uniform sampler2D u_count_4;
uniform sampler2D u_count_5;
uniform sampler2D u_count_6;
uniform sampler2D u_count_7;
uniform int u_leaves;
// The expression in postfix: a leaf by its index, -1 union, -2 difference,
// -3 intersection.
uniform int u_program[64];
uniform int u_length;
uniform uint u_tag;
// The section's half-space, when there is one, drawn as the cut.
uniform int u_half;

layout(location = 0) out vec4 out_colour;
layout(location = 1) out uint out_tag;
layout(location = 2) out float out_done;

bool evaluate(uint inside) {
    bool stack[32];
    int top = 0;
    for (int i = 0; i < u_length; i++) {
        int op = u_program[i];
        if (op >= 0) {
            stack[top] = ((inside >> uint(op)) & 1u) == 1u;
            top += 1;
        } else {
            bool b = stack[top - 1];
            bool a = stack[top - 2];
            top -= 1;
            stack[top - 1] = op == -1 ? (a || b) : (op == -2 ? (a && !b) : (a && b));
        }
    }
    return stack[0];
}

void main() {
    ivec2 at = ivec2(gl_FragCoord.xy);
    float depth = texelFetch(u_layer, at, 0).r;
    if (depth >= 1.0) {
        discard;
    }
    vec4 counts[8];
    counts[0] = texelFetch(u_count_0, at, 0);
    counts[1] = texelFetch(u_count_1, at, 0);
    counts[2] = texelFetch(u_count_2, at, 0);
    counts[3] = texelFetch(u_count_3, at, 0);
    counts[4] = texelFetch(u_count_4, at, 0);
    counts[5] = texelFetch(u_count_5, at, 0);
    counts[6] = texelFetch(u_count_6, at, 0);
    counts[7] = texelFetch(u_count_7, at, 0);
    uint inside = 0u;
    for (int j = 0; j < u_leaves; j++) {
        uint crossings = uint(round(counts[j / 4][j % 4] * 255.0));
        if ((crossings & 1u) == 1u) {
            inside |= 1u << uint(j);
        }
    }
    uint leaf = texelFetch(u_leaf, at, 0).r;
    if (evaluate(inside) == evaluate(inside ^ (1u << leaf))) {
        discard;
    }
    out_colour = texelFetch(u_colour, at, 0);
    out_tag = int(leaf) == u_half ? 0u : u_tag;
    out_done = 1.0;
    gl_FragDepth = depth;
}
"#;
