//! Boxes and shapes cut from them: plates, slots, chamfers, rounds and wedges.

use crate::mesh::Mesh;
use crate::revolve::{chamfer_rect_outline, extrude_frustum_polygon, extrude_stack, rounded_rect_outline};
use std::f64::consts::{FRAC_PI_2, PI};

/// Circumradius of a regular n-gon of the given "diameter", across corners or flats.
pub(crate) fn polygon_radius(sides: u32, diameter: f64, across_flats: bool) -> f64 {
    if across_flats {
        (diameter / 2.0) / (PI / sides as f64).cos()
    } else {
        diameter / 2.0
    }
}

pub fn box_mesh(w: f64, d: f64, h: f64) -> Mesh {
    let outline = [(w / 2.0, -d / 2.0), (w / 2.0, d / 2.0), (-w / 2.0, d / 2.0), (-w / 2.0, -d / 2.0)];
    extrude_frustum_polygon(&outline, &outline, h)
}

pub fn plate_mesh(w: f64, d: f64, thickness: f64) -> Mesh {
    box_mesh(w, d, thickness)
}

/// A plate with rounded corners; radius zero gives exactly `plate_mesh`'s vertices.
pub fn rounded_plate_mesh(w: f64, d: f64, thickness: f64, corner_radius: f64, corner_segments: u32) -> Mesh {
    rounded_box_mesh(w, d, thickness, corner_radius, corner_segments)
}

/// A plate with a radius of its own at each corner, in the order +X-Y, +X+Y, -X+Y, -X-Y. Each is held to
/// half the shorter side, so neighbouring arcs never overlap; a corner of 0 stays square.
pub fn plate_corners_mesh(w: f64, d: f64, thickness: f64, radii: [f64; 4], corner_segments: u32) -> Mesh {
    let seg = corner_segments.max(1);
    let (hw, hd) = (w / 2.0, d / 2.0);
    let corners = [(1.0, -1.0, -FRAC_PI_2), (1.0, 1.0, 0.0), (-1.0, 1.0, FRAC_PI_2), (-1.0, -1.0, PI)];
    let mut outline = Vec::with_capacity((4 * seg) as usize);
    for (&(sx, sy, start), radius) in corners.iter().zip(radii) {
        let r = radius.clamp(0.0, hw.min(hd));
        let (cx, cy) = (sx * (hw - r), sy * (hd - r));
        for s in 0..seg {
            let angle = start + FRAC_PI_2 * (s as f64 / seg as f64);
            outline.push((cx + r * angle.cos(), cy + r * angle.sin()));
        }
    }
    extrude_frustum_polygon(&outline, &outline, thickness)
}

/// A slot (obround): a rectangle with semicircular ends, sized by its real outer extents.
pub fn slot_mesh(length: f64, width: f64, thickness: f64, segments: u32) -> Mesh {
    let outline = rounded_rect_outline(length, width, width / 2.0, (segments / 4).max(2));
    extrude_frustum_polygon(&outline, &outline, thickness)
}

/// Which edges of a box a chamfer cuts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChamferEdges {
    All,
    Vertical,
    TopAndBottom,
}

impl ChamferEdges {
    /// From the registry's choice index, in declaration order.
    pub fn from_index(index: u32) -> ChamferEdges {
        match index {
            1 => ChamferEdges::Vertical,
            2 => ChamferEdges::TopAndBottom,
            _ => ChamferEdges::All,
        }
    }
}

/// A box with edges cut at 45 degrees, printable without supports unlike a fillet.
pub fn chamfered_box_mesh(w: f64, d: f64, h: f64, chamfer: f64, edges: ChamferEdges) -> Mesh {
    chamfered_box_cut_mesh(w, d, h, [chamfer; 3], edges)
}

/// A box with edges cut flat, `cut[axis]` deep along each axis: 45 degrees where two depths agree. A
/// cut goes as deep as the box allows, half its side, where the faces it narrows close to a ridge or
/// a point; where three cuts meet, the corner is left as a triangle, as in a CAD chamfer.
pub fn chamfered_box_cut_mesh(w: f64, d: f64, h: f64, cut: [f64; 3], edges: ChamferEdges) -> Mesh {
    let (hw, hd, hh) = (w / 2.0, d / 2.0, h / 2.0);
    let [cx, cy, cz] = [cut[0].clamp(0.0, hw), cut[1].clamp(0.0, hd), cut[2].clamp(0.0, hh)];
    let (vx, vy) = if edges == ChamferEdges::TopAndBottom { (0.0, 0.0) } else { (cx, cy) };
    let horizontal = edges != ChamferEdges::Vertical && cz > 1e-9 && cx.max(cy) > 1e-9;
    if vx.max(vy) < 1e-9 && !horizontal {
        return box_mesh(w, d, h);
    }
    // Eight points, a corner's pair coinciding where its vertical edge is not cut, so all levels pair.
    let octagon = |hw: f64, hd: f64, vx: f64, vy: f64| {
        vec![
            (hw - vx, -hd),
            (hw, -hd + vy),
            (hw, hd - vy),
            (hw - vx, hd),
            (-hw + vx, hd),
            (-hw, hd - vy),
            (-hw, -hd + vy),
            (-hw + vx, -hd),
        ]
    };
    let middle = octagon(hw, hd, vx, vy);
    if !horizontal {
        return extrude_stack(&[(middle.clone(), -hh), (middle, hh)]);
    }
    // The faces are narrowed by the horizontal cuts, which the vertical ones never reach.
    let face = octagon(hw - cx, hd - cy, 0.0, 0.0);
    extrude_stack(&[(face.clone(), -hh), (middle.clone(), -hh + cz), (middle, hh - cz), (face, hh)])
}

pub fn rounded_box_mesh(w: f64, d: f64, h: f64, corner_radius: f64, corner_segments: u32) -> Mesh {
    let outline = rounded_rect_outline(w, d, corner_radius, corner_segments);
    extrude_frustum_polygon(&outline, &outline, h)
}

/// A box whose vertical edges are rounded or chamfered by the same amount.
pub fn corner_box_mesh(w: f64, d: f64, h: f64, size: f64, corner_segments: u32, chamfered: bool) -> Mesh {
    if chamfered {
        let outline = chamfer_rect_outline(w, d, size);
        extrude_frustum_polygon(&outline, &outline, h)
    } else {
        rounded_box_mesh(w, d, h, size, corner_segments)
    }
}

pub fn wedge_mesh(w: f64, d: f64, h: f64, top_width: f64) -> Mesh {
    wedge_shifted_mesh(w, d, h, top_width, 0.0, false)
}

/// A wedge whose top is `top` wide along X, or along Y when `along_y`, its middle `offset` from the
/// base's along that axis: a ramp when it lines up with one side.
pub fn wedge_shifted_mesh(w: f64, d: f64, h: f64, top: f64, offset: f64, along_y: bool) -> Mesh {
    let rect = |x0: f64, x1: f64, y0: f64, y1: f64| [(x1, y0), (x1, y1), (x0, y1), (x0, y0)];
    let bottom = rect(-w / 2.0, w / 2.0, -d / 2.0, d / 2.0);
    let top = if along_y {
        rect(-w / 2.0, w / 2.0, offset - top / 2.0, offset + top / 2.0)
    } else {
        rect(offset - top / 2.0, offset + top / 2.0, -d / 2.0, d / 2.0)
    };
    extrude_frustum_polygon(&bottom, &top, h)
}
