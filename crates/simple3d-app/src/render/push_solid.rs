//! The solid surface: shaded faces, section caps, and ghosted bodies.

use super::*;
use crate::raster::{Rgba, Vertex};
use crate::view::View;
use simple3d_core::config::DisplayMode;
use simple3d_core::scene::Colour;
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
    section: &[Plane],
) {
    let forward = view.forward();
    // Back-face culling: a closed solid's far side is never visible. Parallel projection gives one
    // eye direction for the whole frame.
    let eye = to_eye(view, Vec3::ZERO);
    extend_in_order(steps, item.mesh.indices.len(), |range, out| {
        for index in range {
            let tri = item.mesh.indices[index];
            // Precomputed with the renderable; `Vec3::ZERO` for a degenerate triangle.
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

/// One triangle as steps: from the projected vertices, or clipped by the section, projecting the
/// new points.
pub(crate) fn push_faces(
    out: &mut Vec<Step>,
    view: &View,
    item: &Renderable,
    screen: &[Vertex],
    tri: [u32; 3],
    section: &[Plane],
    step: impl Fn([Vertex; 3]) -> Step,
) {
    if section.is_empty() {
        out.push(step(tri.map(|corner| screen[corner as usize])));
        return;
    }
    let world = tri.map(|corner| item.mesh.positions[corner as usize]);
    for piece in kept(section, world).triangles() {
        out.push(step(piece.map(|at| to_vertex(view, view.to_view(at)))));
    }
}

/// The section cap, so a cut solid still reads as solid (issue 71). Not back-face culled, since a
/// shell can be looked into from either side. A cut that cannot be closed gets no cap rather than
/// a wrong one.
pub(crate) fn push_cap(
    steps: &mut Vec<Step>,
    view: &View,
    item: &Renderable,
    palette: &Palette,
    section: &[Plane],
    mode: DisplayMode,
) {
    // Each section is capped separately, with other sections' cuts removed from its caps.
    for (index, plane) in section.iter().enumerate() {
        let others: Vec<Plane> =
            section.iter().enumerate().filter(|&(other, _)| other != index).map(|(_, p)| *p).collect();
        push_cap_of(steps, view, item, palette, plane, &others, mode);
    }
}

/// One section's cap, with what `others` cut away taken out of it.
fn push_cap_of(
    steps: &mut Vec<Step>,
    view: &View,
    item: &Renderable,
    palette: &Palette,
    plane: &Plane,
    others: &[Plane],
    mode: DisplayMode,
) {
    // One cap per face the cut opens: the plane, or a window's front and box sides, each trimmed to its face.
    for face in section::faces(plane) {
        let (outlines, tags) = section::tagged_loops(&item.mesh, &face.plane);
        if outlines.is_empty() {
            continue;
        }
        // Filled in every mode that fills; in wireframe the cut is its outline alone.
        if mode != DisplayMode::Wireframe {
            for piece in section::fill(&outlines, face.plane.normal) {
                // A painted body's inside is its colour (issue 114), the rest the palette's cut colour.
                let base = match Colour::from_tag(tags.of_triangle(piece)) {
                    Some(Colour([r, g, b])) => [r, g, b, 255],
                    None => palette.cut,
                };
                let colour = shade(base, face.plane.normal, view.forward(), 255);
                let piece = section::within(&piece, &face.bounds);
                for index in 1..piece.len().saturating_sub(1) {
                    let corners = [piece[0], piece[index], piece[index + 1]];
                    for kept in section::clip_by_all(others, corners).triangles() {
                        steps.push(Step::Triangle {
                            v: kept.map(|at| to_vertex(view, view.to_view(at))),
                            colour,
                            // No body: the cap is not a solid an axis can be inside.
                            tag: 0,
                            write_depth: true,
                        });
                    }
                }
            }
        }
        // The cut's edge line wherever the model's lines are drawn; no crease exists there for the edge pass.
        if mode == DisplayMode::Shaded {
            continue;
        }
        let colour = match mode {
            DisplayMode::Wireframe => palette.wire,
            _ => palette.edge,
        };
        for outline in &outlines {
            for (index, &from) in outline.iter().enumerate() {
                let to = outline[(index + 1) % outline.len()];
                let Some((from, to)) = section::segment_within(from, to, &face.bounds) else { continue };
                for &(from, to) in section::kept_by_all(others, from, to).iter() {
                    steps.push(line_step(view, from, to, colour, MARK_BIAS, 0, true));
                }
            }
        }
    }
}

/// A tool's template (issue 70): shaded like a ghost and hidden by the bodies in front of it, biased
/// towards the eye so a face lying on a body's face does not fight it. Its edges are solid where seen
/// and not drawn where a body hides them: drawn through a body, they made the body look removed.
pub(crate) fn push_template(
    steps: &mut Vec<Step>,
    view: &View,
    item: &Renderable,
    screen: &[Vertex],
    base: Rgba,
    section: &[Plane],
) {
    let forward = view.forward();
    extend_in_order(steps, item.mesh.indices.len(), |range, out| {
        for index in range {
            let normal = item.normals[index];
            if normal == Vec3::ZERO {
                continue;
            }
            let colour = shade(base, normal, forward, base[3]);
            let step = |v: [Vertex; 3]| {
                let v = v.map(|v| Vertex { key: v.key + PREVIEW_BIAS * v.key.abs(), ..v });
                Step::Triangle { v, colour, tag: 0, write_depth: false }
            };
            push_faces(out, view, item, screen, item.mesh.indices[index], section, step);
        }
    });
    let seen = [base[0], base[1], base[2], 255];
    extend_in_order(steps, item.edges.len(), |range, out| {
        for &edge in &item.edges[range] {
            push_edge(out, view, item, screen, edge, section, |a, b| {
                let bias = PREVIEW_BIAS * (a.key.abs() + b.key.abs()) * 0.5;
                Step::Overlay { a, b, colour: seen, bias }
            });
        }
    });
}

/// Ghosts: no culling and no depth writes, so a hidden tool body reads as a translucent volume.
pub(crate) fn push_ghost(
    steps: &mut Vec<Step>,
    view: &View,
    item: &Renderable,
    screen: &[Vertex],
    base: Rgba,
    section: &[Plane],
) {
    let forward = view.forward();
    extend_in_order(steps, item.mesh.indices.len(), |range, out| {
        for index in range {
            let normal = item.normals[index];
            if normal == Vec3::ZERO {
                continue;
            }
            let colour = shade(base, normal, forward, base[3]);
            // A ghost writes no depth, so its tag is never read.
            let step = |v| Step::Triangle { v, colour, tag: 0, write_depth: false };
            push_faces(out, view, item, screen, item.mesh.indices[index], section, step);
        }
    });
}
