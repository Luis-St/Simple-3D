//! Shaders for resident mesh faces and feature lines.

use super::{resident, BOX_COMMON};

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
