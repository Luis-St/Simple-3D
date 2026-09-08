//! A vertex into eye space, and the shade its face is drawn at.

use super::*;
use crate::raster::{Rgba, Vertex};
use crate::view::View;
use simple3d_core::scene::Colour;
use simple3d_geom::section::{self, Plane};
use simple3d_geom::Vec3;

/// Project a world point to a rasterizer vertex, with the depth key the
/// framebuffer expects: larger is nearer, and linear in screen space. Under a
/// parallel projection the view-space depth itself is that, negated.
pub(crate) fn to_vertex(view: &View, view_space: Vec3) -> Vertex {
    let (pos, z) = view.view_to_screen(view_space);
    Vertex { pos, key: -z as f32 }
}

/// Shade one triangle: a headlight from the camera plus a constant fill, so a
/// face turned away from the eye is dim but never black.
pub(crate) fn shade(base: Rgba, normal: Vec3, view_dir: Vec3, alpha: u8) -> Rgba {
    let lambert = normal.dot(-view_dir).abs();
    let factor = 0.34 + 0.66 * lambert;
    [
        (base[0] as f64 * factor).min(255.0) as u8,
        (base[1] as f64 * factor).min(255.0) as u8,
        (base[2] as f64 * factor).min(255.0) as u8,
        alpha,
    ]
}

/// The direction from a triangle towards the eye, for back-face culling.
///
/// The projection is parallel, so there is no eye to point at: every ray runs
/// along the view direction and that direction is the answer for every triangle
/// in the frame. Pointing at `eye - centroid` instead is right only near the
/// middle of the frame, and the further a face sits from it the more the answer
/// tilts, until a face within a few degrees of edge-on is culled although it is
/// facing the viewer -- which is what once dropped the side wall off a plate
/// seen almost from the side.
pub(crate) fn to_eye(view: &View, _centroid: Vec3) -> Vec3 {
    -view.forward()
}

/// The colour one triangle is painted: whatever body it came from, if that
/// body was painted, and the theme's colour for solids otherwise. The tag
/// travels with the surface through every boolean, so the far wall of a hole
/// drilled by a painted cutter is the cutter's colour and the plate around it
/// stays the plate's.
pub(crate) fn triangle_base(item: &Renderable, index: usize, base: Rgba) -> Rgba {
    match Colour::from_tag(item.mesh.tag(index)) {
        Some(Colour([r, g, b])) => [r, g, b, base[3]],
        None => base,
    }
}

/// What is left of one triangle: the whole of it while there is no section,
/// and what the plane leaves of it otherwise.
///
/// Every path that draws a face goes through this, so a section cannot cut the
/// shading and miss the ghosts, or cut the model and miss the glow.
pub(crate) fn kept(section: Option<Plane>, world: [Vec3; 3]) -> section::Clipped {
    match section {
        Some(plane) => section::clip_triangle(&plane, world),
        None => section::Clipped::untouched(world),
    }
}

/// The same question for a line: the stretch of it on the kept side, or nothing
/// when the cut took all of it.
pub(crate) fn kept_line(section: Option<Plane>, a: Vec3, b: Vec3) -> Option<(Vec3, Vec3)> {
    match section {
        Some(plane) => section::clip_segment(&plane, a, b),
        None => Some((a, b)),
    }
}
