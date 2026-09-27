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
    // Silhouette edges are drawn a second time one pixel outwards, since a depth test cannot draw
    // them: the face beside them is nearly edge-on (on a 32-segment sphere 17 of 180 rim sectors were
    // missing, and no bias fixed it). The offset is perpendicular to the edge and away from the front
    // face's centre, and does not thicken the line where it is already drawn.
    let mut push = |edge: [u32; 2], away: Option<Vec3>| {
        let a = item.mesh.positions[edge[0] as usize];
        let b = item.mesh.positions[edge[1] as usize];
        // The outline is cut with the shape, so it never hangs where the model was cut away.
        for (a, b) in kept_line(section, a, b).iter().copied() {
            let tag = item.body_tag(edge[0] as usize, tag_base);
            steps.push(line_step(view, a, b, colour, SELECTION_BIAS, tag, true));
            let Some(away) = away else { continue };
            let (va, vb) = (to_vertex(view, view.to_view(a)), to_vertex(view, view.to_view(b)));
            let along = vb.pos - va.pos;
            let normal = egui::vec2(-along.y, along.x);
            if normal.length() < 1e-6 {
                continue;
            }
            // Outward is decided in screen space, so a foreshortened face cannot get it backwards.
            let inward = to_vertex(view, view.to_view(away)).pos - va.pos;
            let normal = normal / normal.length();
            let out = if egui::vec2(normal.x, normal.y).dot(inward) > 0.0 { -normal } else { normal };
            let shift = |v: Vertex| Vertex { pos: v.pos + out, key: v.key };
            steps.push(Step::Line {
                a: shift(va),
                b: shift(vb),
                colour,
                bias: SELECTION_BIAS * (va.key.abs() + vb.key.abs()) * 0.5,
                tag,
                write_depth: true,
            });
        }
    };
    if item.outline.is_empty() {
        // Without adjacency only creases are available; they have the shape on both sides, so no outward pass.
        for &edge in &item.edges {
            push(edge, None);
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
    for edge in &item.outline {
        if edge.junction {
            continue;
        }
        let [near, far] = edge.faces;
        if near == far || faces_the_eye(near) != faces_the_eye(far) {
            // The face on the shape's side of the edge, whose centre says which way is inward.
            let inside = if faces_the_eye(near) { near } else { far };
            push(edge.ends, centroid(inside));
        } else if creases.wanted(faces_the_eye(near)) && is_corner(near, far) {
            push(edge.ends, None);
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
