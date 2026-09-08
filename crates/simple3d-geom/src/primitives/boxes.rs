//! Boxes and the shapes cut from them: plates, slots, chamfers, rounds and
//! wedges.

use crate::mesh::Mesh;
use crate::revolve::{
    chamfer_rect_outline, extrude_frustum_polygon, extrude_stack, inset_convex_outline, rounded_rect_outline,
};
use std::f64::consts::PI;

/// Circumradius for a regular n-gon of the given "diameter", honouring the
/// across-corners / across-flats measurement convention.
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

/// A plate whose four corners are rounded. A radius of zero gives exactly the
/// vertices `plate_mesh` does, so the flat shape stays an alias of the box.
pub fn rounded_plate_mesh(w: f64, d: f64, thickness: f64, corner_radius: f64, corner_segments: u32) -> Mesh {
    rounded_box_mesh(w, d, thickness, corner_radius, corner_segments)
}

/// A slot: a rectangle whose two ends are semicircles (an obround, or
/// stadium). Its length and width are its real outer extents, so a slot cut
/// with it is the width of a drill of that diameter.
pub fn slot_mesh(length: f64, width: f64, thickness: f64, segments: u32) -> Mesh {
    let outline = rounded_rect_outline(length, width, width / 2.0, (segments / 4).max(2));
    extrude_frustum_polygon(&outline, &outline, thickness)
}

/// The largest chamfer that still leaves a solid, given which edges it cuts.
/// A chamfer bigger than this would fold the outline through itself, so the
/// generator clamps rather than emitting geometry that is not a solid.
pub fn chamfer_limit(w: f64, d: f64, h: f64, edges: ChamferEdges) -> f64 {
    let smaller = w.min(d);
    match edges {
        // Only the outline is cut: it survives until the chamfers meet.
        ChamferEdges::Vertical => smaller / 2.0,
        // The outline is whole but inset by the chamfer at each end.
        ChamferEdges::TopAndBottom => (smaller / 2.0).min(h / 2.0),
        // Both at once: the inset outline has to have room for its own corner
        // cuts, which is what makes this a quarter rather than a half.
        ChamferEdges::All => (smaller / 4.0).min(h / 2.0),
    }
}

/// Which edges of a box a chamfer cuts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChamferEdges {
    All,
    Vertical,
    TopAndBottom,
}

impl ChamferEdges {
    /// The registry stores a choice as an index; this is the order the options
    /// are declared in.
    pub fn from_index(index: u32) -> ChamferEdges {
        match index {
            1 => ChamferEdges::Vertical,
            2 => ChamferEdges::TopAndBottom,
            _ => ChamferEdges::All,
        }
    }
}

/// A box with its edges cut off at 45 degrees -- the edge treatment a printed
/// part usually wants, where a fillet needs supports and a chamfer does not.
pub fn chamfered_box_mesh(w: f64, d: f64, h: f64, chamfer: f64, edges: ChamferEdges) -> Mesh {
    let c = chamfer.max(0.0).min(chamfer_limit(w, d, h, edges));
    let outline = match edges {
        ChamferEdges::TopAndBottom => chamfer_rect_outline(w, d, 0.0),
        _ => chamfer_rect_outline(w, d, c),
    };
    if edges == ChamferEdges::Vertical || c < 1e-9 {
        return extrude_frustum_polygon(&outline, &outline, h);
    }
    let narrowed = inset_convex_outline(&outline, c);
    extrude_stack(&[
        (narrowed.clone(), -h / 2.0),
        (outline.clone(), -h / 2.0 + c),
        (outline, h / 2.0 - c),
        (narrowed, h / 2.0),
    ])
}

pub fn rounded_box_mesh(w: f64, d: f64, h: f64, corner_radius: f64, corner_segments: u32) -> Mesh {
    let outline = rounded_rect_outline(w, d, corner_radius, corner_segments);
    extrude_frustum_polygon(&outline, &outline, h)
}

/// A box whose vertical edges are either rounded or chamfered by the same
/// amount. The two are one shape with one dimension: which of them a part
/// wants is a printing decision, not a modelling one.
pub fn corner_box_mesh(w: f64, d: f64, h: f64, size: f64, corner_segments: u32, chamfered: bool) -> Mesh {
    if chamfered {
        let outline = chamfer_rect_outline(w, d, size);
        extrude_frustum_polygon(&outline, &outline, h)
    } else {
        rounded_box_mesh(w, d, h, size, corner_segments)
    }
}

pub fn wedge_mesh(w: f64, d: f64, h: f64, top_width: f64) -> Mesh {
    let bottom = [(w / 2.0, -d / 2.0), (w / 2.0, d / 2.0), (-w / 2.0, d / 2.0), (-w / 2.0, -d / 2.0)];
    let top = [
        (top_width / 2.0, -d / 2.0),
        (top_width / 2.0, d / 2.0),
        (-top_width / 2.0, d / 2.0),
        (-top_width / 2.0, -d / 2.0),
    ];
    extrude_frustum_polygon(&bottom, &top, h)
}
