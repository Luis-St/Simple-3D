//! The edges a dragged cutter quiets, tested in the line geometry stage (`gpu/quiet.rs`, issue 129):
//! what `render::Quiet` does for the CPU's edges.

/// Whether a point lies on a dragged cutter. The cutters' triangles are three texels each, relative
/// to the table's origin; each placement draws one cutter's range turned and shifted, and a line's
/// own positions reach the table's frame by `u_quiet_offset`.
pub(crate) const QUIET_COMMON: &str = r#"
#define QUIET_MAX 8
uniform int u_quiet;
uniform sampler2D u_quiet_triangles;
uniform int u_quiet_width;
uniform int u_quiet_count;
uniform ivec2 u_quiet_range[QUIET_MAX];
uniform mat3 u_quiet_turn[QUIET_MAX];
uniform vec3 u_quiet_shift[QUIET_MAX];
uniform vec3 u_quiet_offset;
uniform vec3 u_quiet_lo;
uniform vec3 u_quiet_hi;
uniform float u_quiet_near;

vec3 quiet_corner(int texel) {
    return texelFetch(u_quiet_triangles, ivec2(texel % u_quiet_width, texel / u_quiet_width), 0).xyz;
}

// The point of triangle abc closest to p (Ericson, Real-Time Collision Detection, 5.1.5), as
// `closest_on_triangle`.
vec3 closest_on_triangle(vec3 p, vec3 a, vec3 b, vec3 c) {
    vec3 ab = b - a;
    vec3 ac = c - a;
    vec3 ap = p - a;
    float d1 = dot(ab, ap);
    float d2 = dot(ac, ap);
    if (d1 <= 0.0 && d2 <= 0.0) {
        return a;
    }
    vec3 bp = p - b;
    float d3 = dot(ab, bp);
    float d4 = dot(ac, bp);
    if (d3 >= 0.0 && d4 <= d3) {
        return b;
    }
    float vc = d1 * d4 - d3 * d2;
    if (vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0) {
        return a + ab * (d1 / (d1 - d3));
    }
    vec3 cp = p - c;
    float d5 = dot(ab, cp);
    float d6 = dot(ac, cp);
    if (d6 >= 0.0 && d5 <= d6) {
        return c;
    }
    float vb = d5 * d2 - d1 * d6;
    if (vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0) {
        return a + ac * (d2 / (d2 - d6));
    }
    float va = d3 * d6 - d5 * d4;
    if (va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0) {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    float denom = 1.0 / (va + vb + vc);
    return a + ab * (vb * denom) + ac * (vc * denom);
}

bool on_cutter(vec3 own) {
    vec3 p = own + u_quiet_offset;
    if (any(lessThan(p, u_quiet_lo - u_quiet_near)) || any(greaterThan(p, u_quiet_hi + u_quiet_near))) {
        return false;
    }
    for (int k = 0; k < u_quiet_count; k++) {
        int end = u_quiet_range[k].x + u_quiet_range[k].y;
        for (int t = u_quiet_range[k].x; t < end; t++) {
            vec3 a = u_quiet_turn[k] * quiet_corner(3 * t) + u_quiet_shift[k];
            vec3 b = u_quiet_turn[k] * quiet_corner(3 * t + 1) + u_quiet_shift[k];
            vec3 c = u_quiet_turn[k] * quiet_corner(3 * t + 2) + u_quiet_shift[k];
            if (length(closest_on_triangle(p, a, b, c) - p) <= u_quiet_near) {
                return true;
            }
        }
    }
    return false;
}

// Both ends and the middle, as `Quiet::holds`.
bool quieted(vec3 a, vec3 b) {
    return u_quiet == 1 && on_cutter(a) && on_cutter(b) && on_cutter((a + b) * 0.5);
}
"#;
