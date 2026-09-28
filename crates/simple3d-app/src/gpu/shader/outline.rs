//! Shaders for selection outlines and plane crossings, found from the mesh tables.

use super::{resident, TABLES_COMMON};

/// A selected body's outline, found on the card (`push_selection`), one edge per point.
///
/// The geometry stage looks up the edge's two triangles and draws it where the surface turns
/// away across it, or at a corner facing the camera. Every edge is drawn twice, one whole pixel
/// apart across its minor axis, so each is an unbroken band two pixels wide (issue 99).
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
// How many pixels of a face's depth slope the line is pulled forward by, and the most that may
// add as a fraction of its depth (`SELECTION_SLOPE_PIXELS`, `SELECTION_SLOPE_CAP`).
uniform float u_slope_pixels;
uniform float u_slope_cap;
uniform uint u_tag_base;
uniform vec4 u_clip;

out vec4 v_colour;
flat out uint v_tag;
out vec3 v_world;

// The two ends in the mesh's own coordinates, for the fragment's box test.
vec3 world_a;
vec3 world_b;

void face(uint index, out vec3 normal, out vec3 centre, out float slope) {
    uvec3 corners = texelFetch(u_triangles, cell(index), 0).xyz;
    vec3 a = position(corners.x);
    vec3 b = position(corners.y);
    vec3 c = position(corners.z);
    vec3 n = cross(b - a, c - a);
    float length_ = length(n);
    normal = length_ > 0.0 ? n / length_ : vec3(0.0);
    centre = (a + b + c) / 3.0;
    // How fast the face's depth key changes per pixel; nearly edge-on faces change fastest.
    vec3 pa = project(a);
    vec3 pb = project(b);
    vec3 pc = project(c);
    vec2 e1 = pb.xy - pa.xy;
    vec2 e2 = pc.xy - pa.xy;
    float det = e1.x * e2.y - e2.x * e1.y;
    float k1 = pb.z - pa.z;
    float k2 = pc.z - pa.z;
    slope = abs(det) > 1e-6 ? length(vec2(k1 * e2.y - k2 * e1.y, e1.x * k2 - e2.x * k1) / det) : 1e30;
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
    float near_slope, far_slope;
    face(edge.z, near_normal, near_centre, near_slope);
    face(edge.w, far_normal, far_centre, far_slope);
    bool near_faces = dot(near_normal, u_forward) < -u_edge_on;
    bool far_faces = dot(far_normal, u_forward) < -u_edge_on;

    vec3 a = position(edge.x);
    vec3 b = position(edge.y);
    world_a = a;
    world_b = b;
    vec3 sa = project(a);
    vec3 sb = project(b);
    float scale = (abs(sa.z) + abs(sb.z)) * 0.5;
    // Pulled forward by the slope of the faces it borders that are drawn, so a pixel of the line
    // landing on a steep face beside it is not hidden by that face (issue 99).
    float steepest = max(near_faces ? near_slope : 0.0, far_faces ? far_slope : 0.0);
    float bias = u_bias * scale + min(steepest * u_slope_pixels, u_slope_cap * scale);
    float clip_a = dot(u_clip.xyz, a) + u_clip.w;
    float clip_b = dot(u_clip.xyz, b) + u_clip.w;
    uint tag = min(u_tag_base + texelFetch(u_bodies, cell(edge.x), 0).r + 1u, 65535u);

    bool silhouette = edge.z == edge.w || near_faces != far_faces;
    bool crease = (u_all_creases == 1 || near_faces) && dot(near_normal, far_normal) < u_crease;
    if (!silhouette && !crease) {
        return;
    }
    line(sa, sb, clip_a, clip_b, vec2(0.0), bias, tag);
    // The second copy is a whole pixel over across the minor axis: a unit perpendicular shift left
    // holes along diagonals and made some edges look thicker than others (issue 99).
    vec2 along = sb.xy - sa.xy;
    if (length(along) < 1e-6) {
        return;
    }
    vec2 across = abs(along.x) >= abs(along.y) ? vec2(0.0, 1.0) : vec2(1.0, 0.0);
    if (silhouette) {
        // Out of the shape, away from the face centre, since the face beside it is nearly edge-on.
        // The side is judged across the edge's own direction: against the minor axis alone, a
        // slanted edge's centre could fall on the wrong side of an end and hide both copies.
        vec3 inside = near_faces ? near_centre : far_centre;
        vec2 inward = project(inside).xy - sa.xy;
        vec2 normal = vec2(-along.y, along.x);
        vec2 out_ = dot(normal, inward) > 0.0 ? -normal : normal;
        across = dot(across, out_) < 0.0 ? -across : across;
    }
    line(sa, sb, clip_a, clip_b, across, bias, tag);
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
