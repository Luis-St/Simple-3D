//! How a selected body is picked out.

use super::*;
use crate::raster::{Rgba, Vertex};
use crate::view::View;
use simple3d_core::config::DisplayMode;
use simple3d_geom::section::Plane;
use simple3d_geom::Vec3;

/// The selected shape's outline: edges its surface turns away from the camera across, plus edges
/// with no far side. Feature edges drew nothing on a smooth sphere and rings all over a torus.
#[allow(clippy::too_many_arguments)]
pub(crate) fn push_selection(
    steps: &mut Vec<Step>,
    view: &View,
    item: &Renderable,
    colour: Rgba,
    tag_base: u16,
    mode: DisplayMode,
    section: &[Plane],
) {
    // Every edge is drawn twice, the second copy one whole pixel over across its minor axis, so it is
    // an unbroken band two pixels wide (issue 99); a unit perpendicular shift left holes along
    // diagonals and drew silhouettes thicker than creases. Silhouettes shift outwards, away from the
    // front face's centre, since a depth test cannot draw them: the face beside them is nearly
    // edge-on (on a 32-segment sphere 17 of 180 rim sectors were missing, and no bias fixed it).
    // `steepest` is the depth slope of the drawn faces beside the edge, per pixel.
    let mut push = |edge: [u32; 2], away: Option<Vec3>, steepest: f32| {
        let a = item.mesh.positions[edge[0] as usize];
        let b = item.mesh.positions[edge[1] as usize];
        // The outline is cut with the shape, so it never hangs where the model was cut away.
        for (a, b) in kept_line(section, a, b).iter().copied() {
            let tag = item.body_tag(edge[0] as usize, tag_base);
            let (va, vb) = (to_vertex(view, view.to_view(a)), to_vertex(view, view.to_view(b)));
            let scale = (va.key.abs() + vb.key.abs()) * 0.5;
            let bias = SELECTION_BIAS * scale + (steepest * SELECTION_SLOPE_PIXELS).min(SELECTION_SLOPE_CAP * scale);
            steps.push(Step::Line { a: va, b: vb, colour, bias, tag, write_depth: true });
            let along = vb.pos - va.pos;
            if along.length() < 1e-6 {
                continue;
            }
            let mut across = if along.x.abs() >= along.y.abs() { egui::vec2(0.0, 1.0) } else { egui::vec2(1.0, 0.0) };
            if let Some(away) = away {
                // Outward is decided in screen space, so a foreshortened face cannot get it backwards,
                // and across the edge's own direction: against the minor axis alone, a slanted edge's
                // centre could fall on the wrong side of an end and hide both copies.
                let inward = to_vertex(view, view.to_view(away)).pos - va.pos;
                let normal = egui::vec2(-along.y, along.x);
                let out = if normal.dot(inward) > 0.0 { -normal } else { normal };
                if across.dot(out) < 0.0 {
                    across = -across;
                }
            }
            let shift = |v: Vertex| Vertex { pos: v.pos + across, key: v.key };
            steps.push(Step::Line { a: shift(va), b: shift(vb), colour, bias, tag, write_depth: true });
        }
    };
    if item.outline.is_empty() {
        // Without adjacency only creases are available; they have the shape on both sides, so no outward pass.
        for &edge in &item.edges {
            push(edge, None, 0.0);
        }
        return;
    }
    let towards = view.forward();
    // Normals come from the renderable rather than being recomputed every frame.
    let normals = &item.normals;
    // Edge-on counts as turned away, by a margin: exactly perpendicular faces land either side of
    // zero by noise, which once outlined the back of a box and left it outlined on three sides.
    let front: Vec<bool> = normals.iter().map(|normal| normal.dot(towards) < -EDGE_ON).collect();
    let faces_the_eye = |face: u32| front.get(face as usize).copied().unwrap_or(false);
    // Corners are measured from the faces, not taken from the feature edges, which use another angle.
    let cos_limit = SELECTION_CREASE.to_radians().cos();
    let is_corner = |a: u32, b: u32| match (normals.get(a as usize), normals.get(b as usize)) {
        (Some(a), Some(b)) => a.dot(*b) < cos_limit,
        _ => false,
    };
    let centroid = |face: u32| -> Option<Vec3> {
        let tri = item.mesh.indices.get(face as usize)?;
        Some(
            (item.mesh.positions[tri[0] as usize]
                + item.mesh.positions[tri[1] as usize]
                + item.mesh.positions[tri[2] as usize])
                / 3.0,
        )
    };
    // Creases inside the contour are accented too (issue 89), so a selected box reads as an orange
    // box. Only those facing the camera, except in wireframe, which shows the far side on purpose;
    // drawing hidden ones once scribbled a cage over the model. In every mode, and with no outward pass.
    let creases = if mode == DisplayMode::Wireframe { Creases::All } else { Creases::Facing };
    // How fast a face's depth key changes per pixel on screen; nearly edge-on faces change fastest.
    let slope = |face: u32| -> f32 {
        let Some(tri) = item.mesh.indices.get(face as usize) else { return 0.0 };
        let [a, b, c] = tri.map(|i| to_vertex(view, view.to_view(item.mesh.positions[i as usize])));
        let (e1, e2) = (b.pos - a.pos, c.pos - a.pos);
        let det = e1.x * e2.y - e2.x * e1.y;
        if det.abs() < 1e-6 {
            return f32::MAX;
        }
        let (k1, k2) = (b.key - a.key, c.key - a.key);
        egui::vec2(k1 * e2.y - k2 * e1.y, e1.x * k2 - e2.x * k1).length() / det.abs()
    };
    let drawn_slope = |face: u32| if faces_the_eye(face) { slope(face) } else { 0.0 };
    for edge in &item.outline {
        if edge.junction {
            continue;
        }
        let [near, far] = edge.faces;
        let steepest = || drawn_slope(near).max(drawn_slope(far));
        if near == far || faces_the_eye(near) != faces_the_eye(far) {
            // The face on the shape's side of the edge, whose centre says which way is inward.
            let inside = if faces_the_eye(near) { near } else { far };
            push(edge.ends, centroid(inside), steepest());
        } else if creases.wanted(faces_the_eye(near)) && is_corner(near, far) {
            push(edge.ends, None, steepest());
        }
    }
}

/// Which of a selected shape's creases the highlight takes, by display mode (see [`push_selection`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Creases {
    /// Only those on the side facing the camera.
    Facing,
    All,
}

impl Creases {
    pub(super) fn wanted(self, facing: bool) -> bool {
        self == Creases::All || facing
    }
}

pub(crate) fn push_wireframe(
    steps: &mut Vec<Step>,
    view: &View,
    item: &Renderable,
    screen: &[Vertex],
    colour: Rgba,
    section: &[Plane],
) {
    extend_in_order(steps, item.edges.len(), |range, out| {
        for edge in &item.edges[range] {
            // No depth bias and no fill, so the far side shows, as wireframe intends; nothing owns a pixel's
            // depth, so the wireframe tag is used.
            push_edge(out, view, item, screen, *edge, section, |a, b| projected_line_step(a, b, colour, 0.0, 0, true));
        }
    });
}
