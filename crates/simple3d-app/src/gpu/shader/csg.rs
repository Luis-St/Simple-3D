//! The shaders of the per-pixel boolean passes.

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
// A cutter's walls or a section's cap: the colour of what it cuts, when its alpha is set (issue 114).
uniform vec4 u_inherit;
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
    if (u_inherit.a > 0.0) {
        base = vec4(u_inherit.rgb, u_base.a);
    } else if (u_painted == 1) {
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
