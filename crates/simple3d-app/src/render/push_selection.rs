//! How a selected body is picked out.

use super::*;
use crate::raster::{Rgba, Vertex};
use crate::view::View;
use simple3d_core::config::DisplayMode;
use simple3d_geom::section::Plane;
use simple3d_geom::Vec3;

#[allow(clippy::too_many_arguments)]
pub(crate) fn push_selection(
    steps: &mut Vec<Step>,
    view: &View,
    item: &Renderable,
    colour: Rgba,
    tag_base: u16,
    mode: DisplayMode,
    section: Option<Plane>,
) {
    // Drawn a second time, one pixel out from the shape, and that is what makes
    // it a line rather than a row of dots.
    //
    // A silhouette edge is the one line in the frame a depth test cannot draw.
    // It lies exactly where the surface turns away from the eye, so the face
    // beside it is nearly edge-on and its depth changes by more across a single
    // pixel than a bias can cover: measured on a 32-segment sphere, seventeen of
    // a hundred and eighty two-degree sectors of the rim had nothing drawn at
    // all, and raising the bias tenfold -- past where a mark starts showing
    // through the far side of a solid -- still left six. It is not an epsilon
    // problem, and the pixel just outside the silhouette is not covered by the
    // shape at all, so nothing there has to be won from.
    //
    // The offset is perpendicular to the edge on screen and away from the front
    // face's own centre, which for a silhouette edge is out of the shape. It
    // does not thicken the line where it is already drawn -- the two passes land
    // on the same pixel for a nearly-vertical edge -- so a selection still reads
    // as a hairline and not as a halo.
    let mut push = |edge: [u32; 2], away: Option<Vec3>| {
        let a = item.mesh.positions[edge[0] as usize];
        let b = item.mesh.positions[edge[1] as usize];
        // The outline is cut with the shape it outlines: an accent line left
        // hanging in the air where the model has been cut away says the
        // selection is somewhere it no longer is.
        let Some((a, b)) = kept_line(section, a, b) else { return };
        let tag = item.body_tag(edge[0] as usize, tag_base);
        steps.push(line_step(view, a, b, colour, SELECTION_BIAS, tag, true));
        let Some(away) = away else { return };
        let (va, vb) = (to_vertex(view, view.to_view(a)), to_vertex(view, view.to_view(b)));
        let along = vb.pos - va.pos;
        let normal = egui::vec2(-along.y, along.x);
        if normal.length() < 1e-6 {
            return;
        }
        // Which way along that perpendicular leads out of the shape, decided in
        // screen space so a foreshortened face cannot get it backwards.
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
    };
    if item.outline.is_empty() {
        // Prepared without the adjacency -- the creases are what there is, and a
        // crease has an inside on both sides, so no outward pass for it.
        for &edge in &item.edges {
            push(edge, None);
        }
        return;
    }
    let towards = view.forward();
    // Edge-on counts as turned away, and by a margin. A face exactly
    // perpendicular to the view -- every side face of a box seen straight on --
    // has a dot product of zero, and which side of zero the arithmetic lands on
    // is noise: the two triangles of one face can disagree, and neighbouring
    // faces of the same box certainly do. Tested against plain zero, a box in
    // the front view had the right-hand side face come out as facing the eye,
    // which made the front face's own right edge no silhouette at all and the
    // *back* face's right edge one instead -- so the line was drawn at the far
    // side of the box, lost the depth test against the box's own front face,
    // and the shape came back outlined on three sides out of four.
    let normals: Vec<Vec3> = item.mesh.indices.iter().map(|tri| item.mesh.triangle_normal(*tri)).collect();
    let front: Vec<bool> = normals.iter().map(|normal| normal.dot(towards) < -EDGE_ON).collect();
    let faces_the_eye = |face: u32| front.get(face as usize).copied().unwrap_or(false);
    // Whether the surface really turns a corner across an edge, measured from
    // the faces themselves rather than read out of the feature edges the edge
    // pass draws -- those are a different question asked at a different angle.
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
    // The creases inside the contour, in the accent as well (issue 89).
    //
    // The silhouette says where a shape ends; on anything with corners it is
    // the edges *within* that contour -- the three meeting at the near corner
    // of a box -- that say which shape it is, and they stayed in the ordinary
    // edge colour, so a selected box read as an orange ring drawn around a grey
    // box rather than as an orange box.
    //
    // Only the ones facing the camera. The three creases at the *far* corner of
    // a box project inside the same contour, and drawing those is how an
    // earlier attempt at this scribbled a cage over the model: what is hidden
    // by the shape is not part of what the shape looks like. Wireframe is the
    // exception -- it shows the far side of everything on purpose, so a
    // selection that stopped at the near side would be orange in front and grey
    // behind.
    //
    // Every mode, not only the ones that draw edges. In plain shaded the body
    // has no lines of its own and these are the only ones on it, which is
    // exactly the point: they say which shape is selected. In shaded-with-edges
    // they are the very edges already on screen, recoloured.
    //
    // A crease inside the contour gets no outward pass: it has the shape on
    // both sides, so there is no "out" to step to, and stepping either way
    // would only thicken it.
    let creases = if mode == DisplayMode::Wireframe { Creases::All } else { Creases::Facing };
    for edge in &item.outline {
        if edge.junction {
            continue;
        }
        let [near, far] = edge.faces;
        if near == far || faces_the_eye(near) != faces_the_eye(far) {
            // The face on the shape's own side of this edge, whose centre says
            // which way is inward.
            let inside = if faces_the_eye(near) { near } else { far };
            push(edge.ends, centroid(inside));
        } else if creases.wanted(faces_the_eye(near)) && is_corner(near, far) {
            push(edge.ends, None);
        }
    }
}

/// Which of a selected shape's creases the highlight takes, which is decided by
/// the display mode -- see [`push_selection`].
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
    colour: Rgba,
    section: Option<Plane>,
) {
    for edge in &item.edges {
        let a = item.mesh.positions[edge[0] as usize];
        let b = item.mesh.positions[edge[1] as usize];
        let Some((a, b)) = kept_line(section, a, b) else { continue };
        // No depth bias and no filled faces, so the whole wireframe is visible
        // including the far side -- which is the point of wireframe.
        // The tag is the wireframe's own: nothing is filled, so nothing owns a
        // pixel's depth in a way an axis has to see through.
        steps.push(line_step(view, a, b, colour, 0.0, 0, true));
    }
}
