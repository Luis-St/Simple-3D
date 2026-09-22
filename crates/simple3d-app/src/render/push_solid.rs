//! The solid surface: shaded faces, section caps, and ghosted bodies.

use super::*;
use crate::raster::{Rgba, Vertex};
use crate::view::View;
use simple3d_core::config::DisplayMode;
use simple3d_geom::section::{self, Plane};
use simple3d_geom::Vec3;

#[allow(clippy::too_many_arguments)]
pub(crate) fn push_shaded(
    steps: &mut Vec<Step>,
    view: &View,
    item: &Renderable,
    screen: &[Vertex],
    colour_base: Rgba,
    alpha: u8,
    tag_base: u16,
    section: Option<Plane>,
) {
    let forward = view.forward();
    // Back-face culling in world space, where it means something: for a
    // closed solid the far side is never visible, so this halves the work.
    // The projection is parallel, so the direction to the eye is one
    // direction for the whole frame.
    let eye = to_eye(view, Vec3::ZERO);
    extend_in_order(steps, item.mesh.indices.len(), |range, out| {
        for index in range {
            let tri = item.mesh.indices[index];
            // The normal was worked out when the renderable was made: it is the
            // surface's, not the camera's, and `Vec3::ZERO` is what a triangle
            // too degenerate to have one comes back as.
            let normal = item.normals[index];
            if normal == Vec3::ZERO || normal.dot(eye) <= 0.0 {
                continue;
            }
            let colour = shade(triangle_base(item, index, colour_base), normal, forward, alpha);
            let tag = item.tag(index, tag_base);
            push_faces(out, view, item, screen, tri, section, |v| Step::Triangle { v, colour, tag, write_depth: true });
        }
    });
}

/// One triangle as steps: straight from the projected vertices while there is
/// no section, and through the plane when there is -- the pieces it leaves are
/// new points, which have to be projected for themselves.
pub(crate) fn push_faces(
    out: &mut Vec<Step>,
    view: &View,
    item: &Renderable,
    screen: &[Vertex],
    tri: [u32; 3],
    section: Option<Plane>,
    step: impl Fn([Vertex; 3]) -> Step,
) {
    match section {
        None => out.push(step(tri.map(|corner| screen[corner as usize]))),
        Some(_) => {
            let world = tri.map(|corner| item.mesh.positions[corner as usize]);
            for piece in kept(section, world).triangles() {
                out.push(step(piece.map(|at| to_vertex(view, view.to_view(at)))));
            }
        }
    }
}

/// The cap: the cut filled in, so that a sectioned solid still reads as solid
/// and the thickness of a wall can be seen (issue 71).
///
/// Drawn without back-face culling. The cap faces the material that went away,
/// which is where the camera usually is, but a shell can be looked into from
/// the other side as well and a cap that vanished there would put a hole back
/// in the picture the cut was made to remove.
///
/// A cut the geometry cannot close contributes nothing rather than a guess: the
/// section then reads as it did before caps existed, which is a fair way to
/// fail and never a wrong wall.
pub(crate) fn push_cap(
    steps: &mut Vec<Step>,
    view: &View,
    item: &Renderable,
    palette: &Palette,
    section: Option<Plane>,
    mode: DisplayMode,
) {
    let Some(plane) = section else { return };
    let outlines = section::loops(&item.mesh, &plane);
    if outlines.is_empty() {
        return;
    }
    // Filled in every mode that fills anything. Wireframe fills nothing, so
    // there the cut is its outline alone -- without which a wireframe section
    // is a shape that stops for no stated reason.
    if mode != DisplayMode::Wireframe {
        let colour = shade(palette.cut, plane.normal, view.forward(), 255);
        for piece in section::fill(&outlines, plane.normal) {
            steps.push(Step::Triangle {
                v: piece.map(|at| to_vertex(view, view.to_view(at))),
                colour,
                // No body: the cap is not a solid an origin axis can be inside
                // of, and the axis rule is asked about the model, not the cut.
                tag: 0,
                write_depth: true,
            });
        }
    }
    // The line round the cut wherever the mode draws the model's own lines: the
    // edge the cut made is one of them, and the only one with no crease behind
    // it for the edge pass to find.
    if mode == DisplayMode::Shaded {
        return;
    }
    let colour = match mode {
        DisplayMode::Wireframe => palette.wire,
        _ => palette.edge,
    };
    for outline in &outlines {
        for (index, &from) in outline.iter().enumerate() {
            let to = outline[(index + 1) % outline.len()];
            steps.push(line_step(view, from, to, colour, MARK_BIAS, 0, true));
        }
    }
}

/// Ghosts are drawn without back-face culling and without writing depth, so a
/// hidden tool body reads as a translucent volume rather than a flat patch.
pub(crate) fn push_ghost(
    steps: &mut Vec<Step>,
    view: &View,
    item: &Renderable,
    screen: &[Vertex],
    base: Rgba,
    section: Option<Plane>,
) {
    let forward = view.forward();
    extend_in_order(steps, item.mesh.indices.len(), |range, out| {
        for index in range {
            let normal = item.normals[index];
            if normal == Vec3::ZERO {
                continue;
            }
            let colour = shade(base, normal, forward, base[3]);
            // A ghost writes no depth, so the tag it would have written is
            // never read; it carries the one a solid would have had for form's
            // sake.
            let step = |v| Step::Triangle { v, colour, tag: 0, write_depth: false };
            push_faces(out, view, item, screen, item.mesh.indices[index], section, step);
        }
    });
}
