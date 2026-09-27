//! The shader sources, and the vertex they take.

use crate::raster::{Rgba, Vertex};

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

/// Fragment shader for resident mesh lines: `SOLID_SOURCE` plus the per-pixel section test.
pub(crate) fn line_fragment() -> String {
    format!(
        "#version 330 core\n{BOX_COMMON}\n{}",
        r#"
in vec4 v_colour;
flat in uint v_tag;
in vec3 v_world;

layout(location = 0) out vec4 out_colour;
layout(location = 1) out uint out_tag;

void main() {
    if (cut_away(v_world)) {
        discard;
    }
    out_colour = v_colour;
    out_tag = v_tag;
}
"#
    )
}

fn resident(stage: &str) -> String {
    format!("#version 330 core\n{RESIDENT_COMMON}\n{stage}")
}

/// A resident mesh's faces, projected on the card and culled by the pipeline.
///
/// Same arithmetic as `push_shaded`, `push_ghost` and `push_glow`, moved to the card so an orbit
/// uploads only a camera. Normals come from the pixels' own derivatives (see `FACE_FRAGMENT`),
/// and solid back faces are dropped by `GL_CULL_FACE`.
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
// The boolean preview compares depths of the same faces across programs, which requires
// identical vertex placement (`gpu/csg.rs`).
invariant gl_Position;

void main() {
    vec3 pos = u_model * in_pos;
    vec3 screen = project(pos);
    gl_Position = place(screen.xy, screen.z);
    gl_ClipDistance[0] = dot(u_clip.xyz, pos) + u_clip.w;
    // A triangle with any corner hidden is dropped; hidden parts share no corners with others
    // (`Renderable::parts`).
    gl_ClipDistance[1] = hidden(uint(gl_VertexID)) ? -1.0 : 1.0;
    v_pos = pos;
    v_tag = min(u_tag_base + in_body + 1u, 65535u);
}
"#,
    )
}

/// A face's colour. The normal is the cross product of the position's screen derivatives, so
/// none is stored; painted faces read their colour from the paint table by triangle index.
pub(crate) fn face_fragment() -> String {
    format!("#version 330 core\n{BOX_COMMON}\n{FACE_FRAGMENT_BODY}")
}

const FACE_FRAGMENT_BODY: &str = r#"
in vec3 v_pos;
flat in uint v_tag;

uniform vec3 u_forward;
// Default face colour in bytes: the palette's solid, ghost or glow.
uniform vec4 u_base;
// 0 a solid, 1 a ghost, 2 a glow.
uniform int u_mode;
// Whether the mesh has any paint, and the per-triangle colour tag table.
uniform int u_painted;
uniform usampler2D u_paint;
uniform int u_table_width;

layout(location = 0) out vec4 out_colour;
layout(location = 1) out uint out_tag;

void main() {
    if (cut_away(v_pos)) {
        discard;
    }
    vec3 normal = cross(dFdx(v_pos), dFdy(v_pos));
    float length_ = length(normal);
    float facing = length_ > 0.0 ? abs(dot(normal / length_, u_forward)) : 0.0;

    // Paint overrides the colour; alpha is always the base's, as in `triangle_base`.
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
        // `shade`: a headlight plus a constant fill, truncated like the CPU's `as u8`.
        float factor = 0.34 + 0.66 * facing;
        out_colour = vec4(floor(min(base.rgb * factor, vec3(255.0))), base.a) / 255.0;
    }
    out_tag = v_tag;
}
"#;

/// A resident mesh's feature edges, read via the edge list. The geometry stage sees both ends,
/// so the line gets `line_step`'s bias from the mean of their keys.
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
out vec3 g_pos;
out float g_clip;
flat out uint g_tag;
flat out int g_hidden;

void main() {
    vec3 pos = u_model * in_pos;
    g_screen = project(pos);
    g_pos = pos;
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
in vec3 g_pos[];
in float g_clip[];
flat in uint g_tag[];
flat in int g_hidden[];

uniform vec4 u_colour;
uniform float u_bias;

out vec4 v_colour;
flat out uint v_tag;
out vec3 v_world;

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
        v_world = g_pos[i];
        EmitVertex();
    }
    EndPrimitive();
}
"#,
    )
}

/// A selected body's outline, found on the card (`push_selection`), one edge per point.
///
/// The geometry stage looks up the edge's two triangles and draws it where the surface turns
/// away across it, or at a corner facing the camera. Silhouette edges are drawn again one pixel
/// outwards so they form a line rather than dots.
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
out vec3 v_world;

// The two ends in the mesh's own coordinates, for the fragment's box test.
vec3 world_a;
vec3 world_b;

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
    v_world = world_a;
    EmitVertex();
    gl_Position = place(b.xy + shift, b.z + bias);
    gl_ClipDistance[0] = clip_b;
    v_colour = u_colour;
    v_tag = tag;
    v_world = world_b;
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
    world_a = a;
    world_b = b;
    vec3 sa = project(a);
    vec3 sb = project(b);
    float bias = u_bias * (abs(sa.z) + abs(sb.z)) * 0.5;
    float clip_a = dot(u_clip.xyz, a) + u_clip.w;
    float clip_b = dot(u_clip.xyz, b) + u_clip.w;
    uint tag = min(u_tag_base + texelFetch(u_bodies, cell(edge.x), 0).r + 1u, 65535u);

    if (edge.z == edge.w || near_faces != far_faces) {
        line(sa, sb, clip_a, clip_b, vec2(0.0), bias, tag);
        // One pixel out of the shape: perpendicular to the edge, away from the face centre.
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

/// Where planes cross a resident mesh, one segment per triangle per plane: principal plane marks
/// and section cut edges, computed on the card instead of by `snap::plane_crossing` and
/// `section::loops`.
///
/// A corner counts as on a plane within `u_on_plane`, a few times the single-precision error,
/// since the recentred, rounded positions no longer hit the CPU's exact zeros.
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

// Up to three planes, each a distance from the stored position, and its colour.
uniform int u_planes;
uniform vec4 u_plane[3];
uniform vec4 u_plane_colour[3];
uniform float u_on_plane;
uniform float u_bias;
uniform vec4 u_clip;

out vec4 v_colour;
flat out uint v_tag;
out vec3 v_world;

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
        // A triangle lying in the plane has no crossing of its own: its edges are its neighbours'.
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
        v_world = hits[0];
        EmitVertex();
        gl_Position = place(b.xy, b.z + bias);
        gl_ClipDistance[0] = dot(u_clip.xyz, hits[1]) + u_clip.w;
        v_colour = u_plane_colour[k];
        v_tag = 0u;
        v_world = hits[1];
        EmitVertex();
        EndPrimitive();
    }
}
"#,
    )
}

/// The ground grid, computed per pixel over one ground quad instead of `push_grid`'s lines.
///
/// A pixel is on a line when within half a pixel of it across the line on screen. The two levels
/// are `grid_levels`' (the finer faded by its strength, every tenth coarse line in the major
/// colour), and pixels fade with distance from the frame centre. The quad is positioned relative
/// to the coarse level's snapped centre, a multiple of both spacings.
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
// The two spacings, the finer's strength, and each level's reach from the centre.
uniform float u_fine;
uniform float u_coarse;
uniform float u_strength;
uniform vec2 u_half;
// Which coarse line through the centre each way is, in tens, so majors fall on multiples of ten.
uniform vec2 u_major;
uniform vec4 u_minor_colour;
uniform vec4 u_major_colour;
// The distance from the frame centre, in pixels, at which the grid has faded out.
uniform float u_fade;

layout(location = 0) out vec4 out_colour;

// Pixels from the nearest line at `spacing`, measured across it.
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

/// An origin axis arm with its per-pixel rules (`push_axis_line`, `Frame::line_through`).
///
/// Each pixel knows its position along the axis: it fades with distance from the arm's centre,
/// is dropped inside material, and is drawn despite failing the depth test if the winning body is
/// one the axis is arriving at. Material spans come from a table, one row per axis.
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
// Where along the axis the arm starts, and the distance its fade is measured against.
uniform float u_start;
uniform float u_reach;
// The axis direction relative to the view, which decides which side of a body is the approach.
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
        // In front of the whole body, not just one stretch, so a line in a drilled hole stays behind
        // the near wall (`render::body_extents`).
        bool owned = false;
        bool before = true;
        bool after = true;
        for (int i = 0; i < u_count; i++) {
            vec4 span = texelFetch(u_spans, ivec2(i, u_row), 0);
            if (uint(span.z) == owner) {
                owned = true;
                before = before && v_along <= span.x;
                after = after && v_along >= span.y;
            }
        }
        bool arriving = owned && (u_away > 1e-9 ? before : (u_away < -1e-9 ? after : (before || after)));
        if (!arriving) {
            discard;
        }
    }
    out_colour = vec4(u_colour.rgb, u_colour.a * fade);
}
"#;

/// A boolean's edges per pixel: each shape's feature edges, kept only where they are edges of
/// the result. `u_done` holds one past the resolved surface's shape index and its normal.
///
/// The pixel's surface must be the edge's own shape's, and the result must turn or end there.
/// Without the second test, edges buried under coplanar faces showed as dashes.
pub(crate) const CSG_EDGE_FRAGMENT: &str = r#"#version 330 core
in vec4 v_colour;
flat in uint v_tag;

uniform sampler2D u_done;
uniform float u_leaf_code;
uniform uint u_tag;

layout(location = 0) out vec4 out_colour;
layout(location = 1) out uint out_tag;

// The feature edges' own threshold, 20 degrees.
const float SAME = 0.94;

void main() {
    ivec2 at = ivec2(gl_FragCoord.xy);
    vec4 centre = texelFetch(u_done, at, 0);
    if (abs(centre.r - u_leaf_code) > 0.5 / 255.0) {
        discard;
    }
    vec3 normal = centre.gba * 2.0 - 1.0;
    ivec2 last = textureSize(u_done, 0) - 1;
    bool turns = false;
    for (int dy = -1; dy <= 1; dy++) {
        for (int dx = -1; dx <= 1; dx++) {
            vec4 other = texelFetch(u_done, clamp(at + ivec2(dx, dy), ivec2(0), last), 0);
            if (other.r < 0.5 / 255.0 || dot(normal, other.gba * 2.0 - 1.0) < SAME) {
                turns = true;
            }
        }
    }
    if (!turns) {
        discard;
    }
    out_colour = v_colour;
    out_tag = u_tag;
}
"#;

/// The boolean preview's layer pass: at each pixel, the nearest face behind the previous layer,
/// with its colour (as in `FACE_FRAGMENT`) and shape. See `gpu/csg.rs`.
pub(crate) const CSG_PEEL_FRAGMENT: &str = r#"#version 330 core
in vec3 v_pos;

uniform sampler2D u_previous;
uniform int u_first;
// Pixels already resolved, and the rest of the scene's depth: a layer behind it is never seen.
uniform sampler2D u_done;
uniform sampler2D u_scene_depth;
uniform uint u_leaf;
uniform vec3 u_forward;
uniform vec4 u_base;
uniform int u_painted;
uniform usampler2D u_paint;
uniform int u_table_width;
// Plane marks in the shape's own coordinates: a per-pixel boolean has no mesh for the crossing
// pass, so its surface is marked here, a pixel wide like a grid line.
uniform int u_planes;
uniform vec4 u_plane[3];
uniform vec4 u_plane_colour[3];

layout(location = 0) out vec4 out_colour;
layout(location = 1) out uint out_leaf;

void main() {
    ivec2 pixel = ivec2(gl_FragCoord.xy);
    if (texelFetch(u_done, pixel, 0).r > 0.5 / 255.0 || gl_FragCoord.z >= texelFetch(u_scene_depth, pixel, 0).r) {
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
    // The normal turned towards the eye so coplanar faces agree regardless of winding.
    vec3 towards = length_ > 0.0 ? normal / length_ : vec3(0.0, 0.0, 1.0);
    if (dot(towards, u_forward) > 0.0) {
        towards = -towards;
    }
    uvec3 facing_bytes = uvec3(round((towards * 0.5 + 0.5) * 255.0));
    uint leaf_word = u_leaf | (facing_bytes.x << 8) | (facing_bytes.y << 16) | (facing_bytes.z << 24);
    for (int k = 0; k < 3; k++) {
        float d = dot(u_plane[k].xyz, v_pos) + u_plane[k].w;
        float across = length(vec2(dFdx(d), dFdy(d)));
        // A face lying in the plane has no crossing of its own, as in the crossing pass.
        bool lying = length_ > 0.0 && abs(dot(normal / length_, u_plane[k].xyz)) > 0.9999;
        if (k < u_planes && !lying && abs(d) <= 0.5 * across) {
            out_colour = vec4(u_plane_colour[k].rgb, 1.0);
        }
    }
    out_leaf = leaf_word;
}
"#;

/// The boolean preview's counting pass: each face of one shape in front of the layer adds one
/// to that shape's channel; an odd count means inside.
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

/// The boolean preview's packing pass: up to 32 shapes' counts packed as inside bits into the
/// group's channel of the resolve mask.
pub(crate) const CSG_PACK_FRAGMENT: &str = r#"#version 330 core
uniform sampler2D u_count_0;
uniform sampler2D u_count_1;
uniform sampler2D u_count_2;
uniform sampler2D u_count_3;
uniform sampler2D u_count_4;
uniform sampler2D u_count_5;
uniform sampler2D u_count_6;
uniform sampler2D u_count_7;
uniform int u_shapes;

layout(location = 0) out uvec4 out_inside;

void main() {
    ivec2 at = ivec2(gl_FragCoord.xy);
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
    for (int j = 0; j < u_shapes; j++) {
        uint crossings = uint(round(counts[j / 4][j % 4] * 255.0));
        if ((crossings & 1u) == 1u) {
            inside |= 1u << uint(j);
        }
    }
    out_inside = uvec4(inside);
}
"#;

/// The boolean preview's resolving pass: where the layer's point is on the result's surface,
/// it is drawn at the layer's depth and colour, and the pixel is marked done.
pub(crate) const CSG_RESOLVE_FRAGMENT: &str = r#"#version 330 core
uniform sampler2D u_layer;
uniform usampler2D u_leaf;
uniform sampler2D u_colour;
// Which shapes the layer's point is inside, a bit each.
uniform usampler2D u_inside;
// The expression in postfix: a leaf by its index, -1 union, -2 difference, -3 intersection.
uniform int u_program[256];
uniform int u_length;
uniform uint u_tag;
// The first section shape, if any: sections come last and are drawn as the cut.
uniform int u_half;

layout(location = 0) out vec4 out_colour;
layout(location = 1) out uint out_tag;
layout(location = 2) out vec4 out_done;

bool evaluate(uvec4 inside) {
    bool stack[32];
    int top = 0;
    for (int i = 0; i < u_length; i++) {
        int op = u_program[i];
        if (op >= 0) {
            stack[top] = ((inside[op / 32] >> uint(op % 32)) & 1u) == 1u;
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
    uvec4 inside = texelFetch(u_inside, at, 0);
    uint word = texelFetch(u_leaf, at, 0).r;
    uint leaf = word & 255u;
    uvec4 flipped = inside;
    flipped[leaf / 32u] ^= 1u << (leaf % 32u);
    if (evaluate(inside) == evaluate(flipped)) {
        discard;
    }
    out_colour = texelFetch(u_colour, at, 0);
    out_tag = (u_half >= 0 && int(leaf) >= u_half) ? 0u : u_tag;
    // One past the surface's shape index (non-zero means done) and its normal, for the edge pass.
    vec3 normal = vec3(float((word >> 8) & 255u), float((word >> 16) & 255u), float(word >> 24)) / 255.0;
    out_done = vec4(float(leaf + 1u) / 255.0, normal);
    gl_FragDepth = depth;
}
"#;
