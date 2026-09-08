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

/// The origin axes, and the one rule that is not just a depth test.
///
/// A piece of an axis that loses the depth test is still drawn when the body
/// that won that pixel is one this piece is arriving at -- `seen`, indexed by
/// body tag and by which segment of the line this is. That is the whole of
/// `Frame::put_with`'s `through` argument, moved into a shader.
pub(crate) const AXIS_SOURCE: &str = r#"#version 330 core
in vec4 v_colour;
flat in uint v_segment;
in float v_depth;

uniform sampler2D u_depth_tex;
uniform usampler2D u_tag_tex;
uniform sampler2D u_seen;
uniform vec2 u_seen_size;

out vec4 out_colour;

void main() {
    ivec2 at = ivec2(gl_FragCoord.xy);
    float scene = texelFetch(u_depth_tex, at, 0).r;
    if (v_depth >= scene) {
        uint owner = texelFetch(u_tag_tex, at, 0).r;
        if (float(owner) >= u_seen_size.x) discard;
        float allowed = texelFetch(u_seen, ivec2(int(owner), int(v_segment)), 0).r;
        if (allowed < 0.5) discard;
    }
    out_colour = v_colour;
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
