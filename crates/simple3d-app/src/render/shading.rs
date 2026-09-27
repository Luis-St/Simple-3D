//! A vertex into eye space, and the shade its face is drawn at.

use super::*;
use crate::raster::{Rgba, Vertex};
use crate::view::View;
use simple3d_core::scene::Colour;
use simple3d_geom::section::{self, Plane};
use simple3d_geom::Vec3;

/// Project a world point to a rasterizer vertex; the depth key (larger nearer, screen-linear) is
/// the negated view-space depth under parallel projection.
pub(crate) fn to_vertex(view: &View, view_space: Vec3) -> Vertex {
    let (pos, z) = view.view_to_screen(view_space);
    Vertex { pos, key: -z as f32 }
}

/// Every mesh vertex projected once per frame, instead of once per incident triangle; bit-identical.
pub(crate) fn project_all(view: &View, positions: &[Vec3]) -> Vec<Vertex> {
    map_in_order(positions.len(), |index| to_vertex(view, view.to_view(positions[index])))
}

/// Shade one triangle: a camera headlight plus constant fill, so turned-away faces are dim, not black.
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

/// The direction towards the eye for culling: the view direction for every triangle, since the
/// projection is parallel. `eye - centroid` tilted off-centre and culled near-edge-on faces.
pub(crate) fn to_eye(view: &View, _centroid: Vec3) -> Vec3 {
    -view.forward()
}

/// A triangle's colour: its source body's paint, else the theme's solid colour. Tags survive
/// booleans, so a painted cutter colours the walls it drills.
pub(crate) fn triangle_base(item: &Renderable, index: usize, base: Rgba) -> Rgba {
    match Colour::from_tag(item.mesh.tag(index)) {
        Some(Colour([r, g, b])) => [r, g, b, base[3]],
        None => base,
    }
}

/// What is left of one triangle after the sections. Every face path uses this, so no pass is missed.
pub(crate) fn kept(section: &[Plane], world: [Vec3; 3]) -> section::Clipped {
    section::clip_by_all(section, world)
}

/// The same for a line: its kept stretches (none, one, or two around a window).
pub(crate) fn kept_line(section: &[Plane], a: Vec3, b: Vec3) -> section::Segments {
    section::kept_by_all(section, a, b)
}
