//! The ground grid and origin axis shaders.

use super::resident;

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
