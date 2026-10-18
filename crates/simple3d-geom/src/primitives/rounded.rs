//! Slabs rounded along their horizontal edges too: rounded boxes and slots, and slots made hollow.

use super::boxes::{rounded_box_mesh, slot_mesh};
use crate::mesh::Mesh;
use crate::revolve::grid_mesh;
use crate::vec3::Vec3;
use std::f64::consts::{FRAC_PI_2, PI};

/// Which edges of a rounded box are rounded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoundEdges {
    Vertical,
    All,
    VerticalAndTop,
}

impl RoundEdges {
    /// From the registry's choice index, in declaration order.
    pub fn from_index(index: u32) -> RoundEdges {
        match index {
            1 => RoundEdges::All,
            2 => RoundEdges::VerticalAndTop,
            _ => RoundEdges::Vertical,
        }
    }
}

/// A box whose chosen edges are rounded by `radius`, held to what the box has room for. Where three
/// rounded edges meet, the corner is a sphere's eighth.
pub fn rounded_box_edges_mesh(w: f64, d: f64, h: f64, radius: f64, segments: u32, edges: RoundEdges) -> Mesh {
    if edges == RoundEdges::Vertical {
        return rounded_box_mesh(w, d, h, radius, segments);
    }
    let tallest = if edges == RoundEdges::All { h / 2.0 } else { h };
    let r = radius.clamp(0.0, (w / 2.0).min(d / 2.0).min(tallest));
    let rings = slab_rings((w / 2.0 - r, d / 2.0 - r), r, r, h, segments.max(1), edges == RoundEdges::All);
    grid_mesh(&rings, false)
}

/// A slot whose top and bottom edges are rounded by `edge`, hollowed to a racetrack frame by a `wall`
/// short of its half width. Both 0 give exactly [`slot_mesh`].
pub fn slot_shaped_mesh(length: f64, width: f64, thickness: f64, edge: f64, wall: f64, segments: u32) -> Mesh {
    if edge <= 0.0 && wall <= 0.0 {
        return slot_mesh(length, width, thickness, segments);
    }
    let side = length.min(width) / 2.0;
    let core = (length / 2.0 - side, width / 2.0 - side);
    let hollow = wall > 1e-9 && wall < side;
    let mut edge = edge.clamp(0.0, side.min(thickness / 2.0));
    if hollow {
        // Rounded past the wall, the top would close over the hole.
        edge = edge.min(wall);
    }
    let seg = (segments / 4).max(2);
    let mut rings = slab_rings(core, side, edge, thickness, seg, true);
    if !hollow {
        return grid_mesh(&rings, false);
    }
    rings.push(ring(core, side - wall, seg, thickness / 2.0));
    rings.push(ring(core, side - wall, seg, -thickness / 2.0));
    grid_mesh(&rings, true)
}

/// The levels of a slab `h` tall whose outline is its `core` rectangle (half sizes) grown by `side`,
/// bottom to top, with the top edges, and the bottom ones if asked, rounded by `edge` in `seg` steps.
fn slab_rings(core: (f64, f64), side: f64, edge: f64, h: f64, seg: u32, round_bottom: bool) -> Vec<Vec<Vec3>> {
    let hh = h / 2.0;
    let mut rings = Vec::new();
    if round_bottom {
        for k in 0..=seg {
            let phi = -FRAC_PI_2 + FRAC_PI_2 * k as f64 / seg as f64;
            rings.push(ring(core, side - edge + edge * phi.cos(), seg, -hh + edge + edge * phi.sin()));
        }
    } else {
        rings.push(ring(core, side, seg, -hh));
    }
    for k in 0..=seg {
        let phi = FRAC_PI_2 * k as f64 / seg as f64;
        rings.push(ring(core, side - edge + edge * phi.cos(), seg, hh - edge + edge * phi.sin()));
    }
    rings
}

/// The `core` rectangle grown by `radius` at height `z`: a quarter arc of `seg` steps at each corner,
/// both ends included so every ring of a slab pairs point by point.
fn ring(core: (f64, f64), radius: f64, seg: u32, z: f64) -> Vec<Vec3> {
    let corners = [(1.0, -1.0, -FRAC_PI_2), (1.0, 1.0, 0.0), (-1.0, 1.0, FRAC_PI_2), (-1.0, -1.0, PI)];
    let mut out = Vec::with_capacity(4 * (seg as usize + 1));
    for (sx, sy, start) in corners {
        for s in 0..=seg {
            let angle = start + FRAC_PI_2 * s as f64 / seg as f64;
            out.push(Vec3::new(sx * core.0 + radius * angle.cos(), sy * core.1 + radius * angle.sin(), z));
        }
    }
    out
}
