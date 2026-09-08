//! Previews, glows and the edges of a body.

use super::*;
use crate::raster::Rgba;
use crate::view::View;
use simple3d_geom::section::Plane;
use simple3d_geom::Vec3;

/// A tool's preview loops, over the model and depth-tested against it.
///
/// Drawn as [`Step::Overlay`]: after the model, tested against it, and claiming
/// nothing of its own -- so a loop is hidden by the solid it is behind and does
/// not hide the next loop where two of them cross.
pub(crate) fn push_preview(
    steps: &mut Vec<Step>,
    view: &View,
    loops: &[Vec<Vec3>],
    colour: Rgba,
    section: Option<Plane>,
) {
    for loop_ in loops {
        for (index, &from) in loop_.iter().enumerate() {
            let to = loop_[(index + 1) % loop_.len()];
            let Some((from, to)) = kept_line(section, from, to) else { continue };
            let Step::Line { a, b, bias, .. } = line_step(view, from, to, colour, PREVIEW_BIAS, 0, false) else {
                unreachable!("a line step is a line");
            };
            steps.push(Step::Overlay { a, b, colour, bias });
        }
    }
}

/// A body's faces, blended over the finished picture whatever is in front of
/// them (issue 82).
///
/// Every triangle, not only the ones facing the eye: a solid seen through
/// another solid reads as a shape, and half a shell reads as a hole in one.
/// The colour is translucent, so what it is inside is still visible through the
/// glow -- which is how the glow says *where* rather than merely *that*.
pub(crate) fn push_glow(steps: &mut Vec<Step>, view: &View, item: &Renderable, colour: Rgba, section: Option<Plane>) {
    for tri in &item.mesh.indices {
        let world = [
            item.mesh.positions[tri[0] as usize],
            item.mesh.positions[tri[1] as usize],
            item.mesh.positions[tri[2] as usize],
        ];
        for piece in kept(section, world).triangles() {
            steps.push(Step::Glow { v: piece.map(|at| to_vertex(view, view.to_view(at))), colour });
        }
    }
}

pub(crate) fn push_edges(
    steps: &mut Vec<Step>,
    view: &View,
    item: &Renderable,
    colour: Rgba,
    tag_base: u16,
    section: Option<Plane>,
) {
    for edge in &item.edges {
        let a = item.mesh.positions[edge[0] as usize];
        let b = item.mesh.positions[edge[1] as usize];
        let Some((a, b)) = kept_line(section, a, b) else { continue };
        // Tagged like the faces it creases, so an edge of the solid an axis
        // goes into does not hide that axis where the faces either side of it
        // do not.
        let tag = item.body_tag(edge[0] as usize, tag_base);
        steps.push(line_step(view, a, b, colour, EDGE_BIAS, tag, true));
    }
}

/// The selected shape's *outline*: the edges its surface turns away from the
/// camera across, plus any edge with no far side at all.
///
/// It used to draw the feature edges instead, and that swung between the two
/// opposite failures. A sphere at the stock 32 segments creases at 11.25
/// degrees, under the 20-degree threshold, so it has no feature edges and
/// selecting one drew *nothing* -- with only the manipulator in the frame,
/// nothing said what was selected. A torus at the same segment count creases
/// past the threshold around its tube, so selecting one scribbled concentric
/// rings over the whole surface. A silhouette is the same picture for both, and
/// it is what the word outline means.
/// How far a face has to be turned towards the eye before the outline counts it
/// as facing it. Anything flatter than this is edge-on, where the sign of the
/// dot product is arithmetic noise rather than an answer.
pub(crate) const EDGE_ON: f64 = 1e-6;

/// How sharp a crease has to be before the highlight reads it as a corner of
/// the shape rather than as a step in how the shape happens to be tessellated.
///
/// Higher than the twenty degrees the edge pass draws at, and deliberately so.
/// A torus at the stock 32 segments has a sixteen-sided tube, so its rings meet
/// at 22.5 degrees and are feature edges by that measure: highlighted, they
/// scribbled concentric rings across the whole visible surface of a selected
/// torus -- the very picture an earlier pass at the outline was rejected for.
/// Thirty-five degrees clears that by a wide margin and still keeps every real
/// corner: ninety for a box or a cylinder's rim, sixty for a hexagonal prism,
/// forty-five for an octagonal one. A shape faceted coarser than that has
/// corners worth pointing at.
pub(crate) const SELECTION_CREASE: f64 = 35.0;
