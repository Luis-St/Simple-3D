//! Shapes for printed fasteners (issue 123): hex nuts and standoffs, and screw holes to cut with.

use crate::mesh::Mesh;
use crate::revolve::{extrude_frustum_polygon, revolve_open_profile, ring_outline};
use crate::vec3::Vec3;

/// A hexagonal prism `across_flats` wide with a round hole through it, `height` tall, a corner on
/// +X. A hole of 0 leaves it solid; one too wide for the flats is narrowed to leave a wall.
pub fn hex_nut_mesh(across_flats: f64, hole_diameter: f64, height: f64, segments: u32) -> Mesh {
    let corner = across_flats / 2.0 / (std::f64::consts::PI / 6.0).cos();
    let hexagon = ring_outline(6, corner, corner);
    let hole = hole_diameter.min(across_flats * 0.95) / 2.0;
    if hole <= 1e-6 {
        return extrude_frustum_polygon(&hexagon, &hexagon, height);
    }
    // A multiple of six, so the hexagon's corners are among the outline's points.
    let n = segments.max(6).div_ceil(6) * 6;
    let inner = ring_outline(n, hole, hole);
    // The hexagon met along each of the hole's directions, so the two outlines pair point by point.
    let outer: Vec<(f64, f64)> = inner
        .iter()
        .map(|&(x, y)| {
            let angle = y.atan2(x);
            let sector = (angle / (std::f64::consts::PI / 3.0)).floor() * (std::f64::consts::PI / 3.0);
            let flat = across_flats / 2.0 / (angle - sector - std::f64::consts::PI / 6.0).cos();
            (flat * angle.cos(), flat * angle.sin())
        })
        .collect();
    extrude_ring(&outer, &inner, height)
}

/// Which head a screw hole has.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScrewHead {
    /// A cone widening to the head diameter at the top, `angle` degrees across.
    Countersunk {
        angle: f64,
    },
    /// A step to the head diameter, `depth` below the top.
    Counterbored {
        depth: f64,
    },
    Plain,
}

/// A screw hole as a solid to cut with: a shaft `length` long, widening at the top to the head.
/// A head narrower than the shaft is taken as the shaft.
pub fn screw_hole_mesh(shaft_diameter: f64, head_diameter: f64, length: f64, head: ScrewHead, segments: u32) -> Mesh {
    screw_hole_shaped_mesh(shaft_diameter, head_diameter, length, head, 0.0, false, segments)
}

/// [`screw_hole_mesh`] with the head sunk a further `recess` below the surface, and at the bottom if
/// `head_at_bottom`. A head too big for the length swallows the shaft rather than changing shape: a
/// countersink keeps its angle and diameter and starts below the shaft's width, a counterbore runs
/// the whole length.
pub fn screw_hole_shaped_mesh(
    shaft_diameter: f64,
    head_diameter: f64,
    length: f64,
    head: ScrewHead,
    recess: f64,
    head_at_bottom: bool,
    segments: u32,
) -> Mesh {
    let (r, bottom, top) = (shaft_diameter / 2.0, -length / 2.0, length / 2.0);
    let wide = head_diameter.max(shaft_diameter) / 2.0;
    let profile = match head {
        _ if wide - r <= 1e-6 => vec![(r, bottom), (r, top)],
        ScrewHead::Plain => vec![(r, bottom), (r, top)],
        ScrewHead::Countersunk { angle } => {
            let slope = (angle.clamp(1.0, 179.0) / 2.0).to_radians().tan();
            let recess = recess.clamp(0.0, length);
            let seat = top - recess;
            let room = seat - bottom;
            let sink = (wide - r) / slope;
            let mut profile = if sink <= room {
                vec![(r, bottom), (r, seat - sink), (wide, seat)]
            } else {
                vec![(wide - room * slope, bottom), (wide, seat)]
            };
            if recess > 1e-9 {
                profile.push((wide, top));
            }
            profile
        }
        ScrewHead::Counterbored { depth } if depth >= length => vec![(wide, bottom), (wide, top)],
        ScrewHead::Counterbored { depth } => {
            let step = top - depth.max(1e-3);
            vec![(r, bottom), (r, step), (wide, step), (wide, top)]
        }
    };
    let profile: Vec<(f64, f64)> =
        if head_at_bottom { profile.iter().rev().map(|&(r, z)| (r, -z)).collect() } else { profile };
    revolve_open_profile(&profile, segments, true, true)
}

/// A solid between two closed outlines paired point by point, `outer` around `inner`, both
/// counter-clockwise and centred on Z, from -h/2 to +h/2.
pub(crate) fn extrude_ring(outer: &[(f64, f64)], inner: &[(f64, f64)], h: f64) -> Mesh {
    debug_assert_eq!(outer.len(), inner.len());
    let at = |&(x, y): &(f64, f64), z: f64| Vec3::new(x, y, z);
    let (lo, hi) = (-h / 2.0, h / 2.0);
    let n = outer.len();
    let mut mesh = Mesh::new();
    for i in 0..n {
        let j = (i + 1) % n;
        let (o0, o1, i0, i1) = (&outer[i], &outer[j], &inner[i], &inner[j]);
        // Outside wall, top and bottom; then, unless two outer points share a direction and so an
        // inner point, the hole's wall and the rest of top and bottom.
        mesh.push_triangle(at(o0, lo), at(o1, lo), at(o1, hi));
        mesh.push_triangle(at(o0, lo), at(o1, hi), at(o0, hi));
        mesh.push_triangle(at(o0, hi), at(o1, hi), at(i1, hi));
        mesh.push_triangle(at(o0, lo), at(i1, lo), at(o1, lo));
        if (i0.0 - i1.0).hypot(i0.1 - i1.1) <= 1e-9 {
            continue;
        }
        mesh.push_triangle(at(i0, lo), at(i1, hi), at(i1, lo));
        mesh.push_triangle(at(i0, lo), at(i0, hi), at(i1, hi));
        mesh.push_triangle(at(o0, hi), at(i1, hi), at(i0, hi));
        mesh.push_triangle(at(o0, lo), at(i0, lo), at(i1, lo));
    }
    mesh
}
