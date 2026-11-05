//! Smooth-shaded faces for the software renderer: a colour per corner, blended per pixel.

use super::*;
use crate::raster::{Rgba, Vertex};
use crate::view::View;
use simple3d_geom::section::Plane;
use simple3d_geom::Vec3;

/// One triangle with `colours` at its corners as steps, from the projected vertices or clipped by
/// the section; a clipped piece's new corners take the colour mixed at their place in the triangle.
/// Shaded per corner and blended rather than per pixel, as the GPU does, since the software path
/// cannot afford a normal per pixel; the difference is within a shade on a fine mesh.
#[allow(clippy::too_many_arguments)]
pub(crate) fn push_blended(
    out: &mut Vec<Step>,
    view: &View,
    item: &Renderable,
    screen: &[Vertex],
    tri: [u32; 3],
    section: &[Plane],
    colours: [Rgba; 3],
    tag: u16,
) {
    if section.is_empty() {
        let v = tri.map(|corner| screen[corner as usize]);
        out.push(Step::Blended { v, colours, tag, write_depth: true });
        return;
    }
    let world = tri.map(|corner| item.mesh.positions[corner as usize]);
    for piece in kept(section, world).triangles() {
        let v = piece.map(|at| to_vertex(view, view.to_view(at)));
        out.push(Step::Blended { v, colours: piece.map(|at| mixed(world, colours, at)), tag, write_depth: true });
    }
}

/// The colour at `at`, a point of the triangle `world`, mixed from its corners' `colours`.
fn mixed(world: [Vec3; 3], colours: [Rgba; 3], at: Vec3) -> Rgba {
    let (e1, e2, to) = (world[1] - world[0], world[2] - world[0], at - world[0]);
    let (d11, d12, d22) = (e1.dot(e1), e1.dot(e2), e2.dot(e2));
    let (t1, t2) = (to.dot(e1), to.dot(e2));
    let det = d11 * d22 - d12 * d12;
    if det.abs() < 1e-18 {
        return colours[0];
    }
    let b = ((d22 * t1 - d12 * t2) / det).clamp(0.0, 1.0);
    let c = ((d11 * t2 - d12 * t1) / det).clamp(0.0, 1.0);
    let weights = [(1.0 - b - c).max(0.0), b, c];
    std::array::from_fn(|channel| {
        let sum: f64 = (0..3).map(|k| f64::from(colours[k][channel]) * weights[k]).sum();
        sum.round().clamp(0.0, 255.0) as u8
    })
}
