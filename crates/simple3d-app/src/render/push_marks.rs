//! Previews, glows and the edges of a body.

use super::*;
use crate::raster::{Rgba, Vertex};
use crate::view::View;
use simple3d_geom::section::Plane;
use simple3d_geom::Vec3;

/// A tool's preview loops as [`Step::Overlay`]: after the model, depth-tested, writing nothing, so
/// loops hide behind solids but not behind each other.
pub(crate) fn push_preview(steps: &mut Vec<Step>, view: &View, loops: &[Vec<Vec3>], colour: Rgba, section: &[Plane]) {
    for loop_ in loops {
        for (index, &from) in loop_.iter().enumerate() {
            let to = loop_[(index + 1) % loop_.len()];
            for (from, to) in kept_line(section, from, to).iter().copied() {
                let Step::Line { a, b, bias, .. } = line_step(view, from, to, colour, PREVIEW_BIAS, 0, false) else {
                    unreachable!("a line step is a line");
                };
                steps.push(Step::Overlay { a, b, colour, bias });
            }
        }
    }
}

/// A body's faces blended over the finished picture regardless of what is in front (issue 82).
/// Every triangle, since half a shell reads as a hole; translucent, so the surroundings show.
pub(crate) fn push_glow(
    steps: &mut Vec<Step>,
    view: &View,
    item: &Renderable,
    screen: &[Vertex],
    colour: Rgba,
    section: &[Plane],
) {
    extend_in_order(steps, item.mesh.indices.len(), |range, out| {
        for index in range {
            let step = |v| Step::Glow { v, colour };
            push_faces(out, view, item, screen, item.mesh.indices[index], section, step);
        }
    });
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn push_edges(
    steps: &mut Vec<Step>,
    view: &View,
    item: &Renderable,
    screen: &[Vertex],
    colour: Rgba,
    tag_base: u16,
    section: &[Plane],
) {
    let towards = view.forward();
    // The depth slope of a face beside an edge, if it is drawn: solids are culled facing away.
    let drawn_slope = |face: u32| -> f32 {
        let (Some(tri), Some(normal)) = (item.mesh.indices.get(face as usize), item.normals.get(face as usize)) else {
            return 0.0;
        };
        if normal.dot(towards) >= -EDGE_ON {
            return 0.0;
        }
        depth_slope(tri.map(|corner| screen[corner as usize]))
    };
    extend_in_order(steps, item.edges.len(), |range, out| {
        for index in range {
            let edge = item.edges[index];
            // Tagged like its faces, so an edge of the solid an axis enters does not hide that axis.
            let tag = item.body_tag(edge[0] as usize, tag_base);
            let steepest =
                item.edge_faces.get(index).map_or(0.0, |&[near, far]| drawn_slope(near).max(drawn_slope(far)));
            push_edge(out, view, item, screen, edge, section, |a, b| {
                let scale = (a.key.abs() + b.key.abs()) * 0.5;
                let bias = EDGE_BIAS * scale + (steepest * EDGE_SLOPE_PIXELS).min(EDGE_SLOPE_CAP * scale);
                Step::Line { a, b, colour, bias, tag, write_depth: true }
            });
        }
    });
}

/// One mesh edge as a step: from projected vertices, or cut by the section when there is one.
pub(crate) fn push_edge(
    out: &mut Vec<Step>,
    view: &View,
    item: &Renderable,
    screen: &[Vertex],
    edge: [u32; 2],
    section: &[Plane],
    step: impl Fn(Vertex, Vertex) -> Step,
) {
    if section.is_empty() {
        out.push(step(screen[edge[0] as usize], screen[edge[1] as usize]));
        return;
    }
    let (a, b) = (item.mesh.positions[edge[0] as usize], item.mesh.positions[edge[1] as usize]);
    for (a, b) in kept_line(section, a, b).iter().copied() {
        out.push(step(to_vertex(view, view.to_view(a)), to_vertex(view, view.to_view(b))));
    }
}

/// How far a face must turn towards the eye to count as facing it; flatter is edge-on noise.
pub(crate) const EDGE_ON: f64 = 1e-6;

/// How sharp a crease must be for the highlight to treat it as a corner. Above the edge pass's 20
/// degrees so a 32-segment torus's 22.5-degree rings are not scribbled over; 35 keeps every real
/// corner.
pub(crate) const SELECTION_CREASE: f64 = 35.0;
