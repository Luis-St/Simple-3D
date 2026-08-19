//! One generator function per primitive type from the spec (section 3.2).
//! Every shape is generated centred on its own local origin (the default
//! "centre" anchor); `Mesh::apply_base_anchor` converts to the "base" anchor
//! afterward. Every shape's stated dimensions are exact outer bounding
//! dimensions in its own frame, before rotation, per the spec's precision
//! requirement -- on flat axes exactly, on curved axes to within tessellation
//! (vertices lie exactly on the circumscribed circle/sphere).

use crate::mesh::Mesh;
use crate::polyhedra::{self, SizeMode};
use crate::revolve::{
    chamfer_rect_outline, extrude_frustum_polygon, extrude_stack, inset_convex_outline, revolve_closed_profile,
    revolve_open_profile, ring_outline, rounded_rect_outline, sector_outline,
};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

/// Circumradius for a regular n-gon of the given "diameter", honouring the
/// across-corners / across-flats measurement convention.
fn polygon_radius(sides: u32, diameter: f64, across_flats: bool) -> f64 {
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

pub fn regular_prism_mesh(sides: u32, diameter: f64, height: f64, across_flats: bool) -> Mesh {
    let r = polygon_radius(sides.max(3), diameter, across_flats);
    let outline = ring_outline(sides.max(3), r, r);
    extrude_frustum_polygon(&outline, &outline, height)
}

pub fn cone_mesh(bottom_diameter: f64, top_diameter: f64, height: f64, segments: u32) -> Mesh {
    cone_sector_mesh(bottom_diameter, top_diameter, height, segments, 360.0)
}

/// A cone swept through part of a turn: short of a full 360 the two cut faces
/// close it, so a quarter cone is still a solid.
pub fn cone_sector_mesh(
    bottom_diameter: f64,
    top_diameter: f64,
    height: f64,
    segments: u32,
    sweep_deg: f64,
) -> Mesh {
    let n = segments.max(3);
    let bottom = sector_outline(n, bottom_diameter / 2.0, bottom_diameter / 2.0, sweep_deg);
    let top = sector_outline(n, top_diameter / 2.0, top_diameter / 2.0, sweep_deg);
    extrude_frustum_polygon(&bottom, &top, height)
}

pub fn pyramid_mesh(base_w: f64, base_d: f64, top_w: f64, top_d: f64, height: f64) -> Mesh {
    let bottom = [
        (base_w / 2.0, -base_d / 2.0),
        (base_w / 2.0, base_d / 2.0),
        (-base_w / 2.0, base_d / 2.0),
        (-base_w / 2.0, -base_d / 2.0),
    ];
    let top = [
        (top_w / 2.0, -top_d / 2.0),
        (top_w / 2.0, top_d / 2.0),
        (-top_w / 2.0, top_d / 2.0),
        (-top_w / 2.0, -top_d / 2.0),
    ];
    extrude_frustum_polygon(&bottom, &top, height)
}

pub fn regular_pyramid_mesh(
    sides: u32,
    base_diameter: f64,
    top_diameter: f64,
    height: f64,
    across_flats: bool,
) -> Mesh {
    let sides = sides.max(3);
    let rb = polygon_radius(sides, base_diameter, across_flats);
    let rt = polygon_radius(sides, top_diameter, across_flats);
    let bottom = ring_outline(sides, rb, rb);
    let top = ring_outline(sides, rt, rt);
    extrude_frustum_polygon(&bottom, &top, height)
}

pub fn cylinder_mesh(dx: f64, dy: f64, height: f64, segments: u32) -> Mesh {
    cylinder_sector_mesh(dx, dy, height, segments, 360.0)
}

/// A cylinder swept through part of a turn -- a pie slice, capped by the two
/// flat faces where it was cut.
pub fn cylinder_sector_mesh(dx: f64, dy: f64, height: f64, segments: u32, sweep_deg: f64) -> Mesh {
    let outline = sector_outline(segments.max(3), dx / 2.0, dy / 2.0, sweep_deg);
    extrude_frustum_polygon(&outline, &outline, height)
}

pub fn disc_mesh(dx: f64, dy: f64, thickness: f64, segments: u32) -> Mesh {
    cylinder_mesh(dx, dy, thickness, segments)
}

pub fn disc_sector_mesh(dx: f64, dy: f64, thickness: f64, segments: u32, sweep_deg: f64) -> Mesh {
    cylinder_sector_mesh(dx, dy, thickness, segments, sweep_deg)
}

pub fn tube_mesh(outer_diameter: f64, inner_diameter: f64, height: f64, segments: u32) -> Mesh {
    tube_sector_mesh(outer_diameter, inner_diameter, height, segments, 360.0)
}

/// A tube swept through part of a turn: a curved channel or a pipe elbow,
/// closed at both ends by the faces it was cut on.
pub fn tube_sector_mesh(
    outer_diameter: f64,
    inner_diameter: f64,
    height: f64,
    segments: u32,
    sweep_deg: f64,
) -> Mesh {
    let ri = inner_diameter / 2.0;
    let ro = outer_diameter / 2.0;
    let profile = [(ri, -height / 2.0), (ro, -height / 2.0), (ro, height / 2.0), (ri, height / 2.0)];
    revolve_closed_profile(&profile, segments.max(3), sweep_deg)
}

pub fn ring_mesh(outer_diameter: f64, inner_diameter: f64, thickness: f64, segments: u32) -> Mesh {
    tube_mesh(outer_diameter, inner_diameter, thickness, segments)
}

pub fn ring_sector_mesh(
    outer_diameter: f64,
    inner_diameter: f64,
    thickness: f64,
    segments: u32,
    sweep_deg: f64,
) -> Mesh {
    tube_sector_mesh(outer_diameter, inner_diameter, thickness, segments, sweep_deg)
}

pub fn torus_mesh(ring_diameter: f64, tube_diameter: f64, sweep_deg: f64, segments: u32) -> Mesh {
    let major_r = ring_diameter / 2.0;
    let tube_r = tube_diameter / 2.0;
    let minor_segments = (segments / 2).max(3);
    let profile: Vec<(f64, f64)> = (0..minor_segments)
        .map(|i| {
            let phi = TAU * i as f64 / minor_segments as f64;
            (major_r + tube_r * phi.cos(), tube_r * phi.sin())
        })
        .collect();
    revolve_closed_profile(&profile, segments.max(3), sweep_deg)
}

pub fn ellipsoid_mesh(dx: f64, dy: f64, dz: f64, segments: u32) -> Mesh {
    let (rx, ry, rz) = (dx / 2.0, dy / 2.0, dz / 2.0);
    let n = segments.max(3);
    let lat = (segments / 2).max(2);
    let mut mesh = Mesh::new();
    let point = |theta: f64, phi: f64| {
        crate::vec3::Vec3::new(rx * phi.cos() * theta.cos(), ry * phi.cos() * theta.sin(), rz * phi.sin())
    };
    let rings: Vec<Vec<_>> = (0..n)
        .map(|j| {
            let theta = TAU * j as f64 / n as f64;
            (0..=lat).map(|i| point(theta, -FRAC_PI_2 + PI * i as f64 / lat as f64)).collect()
        })
        .collect();
    for j in 0..n as usize {
        let j2 = (j + 1) % n as usize;
        for i in 0..lat as usize {
            let (a, b, c, d) = (rings[j][i], rings[j2][i], rings[j2][i + 1], rings[j][i + 1]);
            mesh.push_triangle(a, b, c);
            mesh.push_triangle(a, c, d);
        }
    }
    mesh
}

pub fn spherical_cap_mesh(diameter: f64, cap_height: f64, segments: u32) -> Mesh {
    let r = diameter / 2.0;
    let ch = cap_height.clamp(1e-6, diameter);
    let phi_cut = ((r - ch) / r).clamp(-1.0, 1.0).asin();
    let zc = ch / 2.0 - r;
    let n = segments.max(3);
    let lat = (segments / 2).max(2);
    let mut mesh = Mesh::new();
    let point = |theta: f64, phi: f64| {
        crate::vec3::Vec3::new(r * phi.cos() * theta.cos(), r * phi.cos() * theta.sin(), r * phi.sin() + zc)
    };
    let rings: Vec<Vec<_>> = (0..n)
        .map(|j| {
            let theta = TAU * j as f64 / n as f64;
            (0..=lat).map(|i| point(theta, phi_cut + (FRAC_PI_2 - phi_cut) * i as f64 / lat as f64)).collect()
        })
        .collect();
    for j in 0..n as usize {
        let j2 = (j + 1) % n as usize;
        for i in 0..lat as usize {
            let (a, b, c, d) = (rings[j][i], rings[j2][i], rings[j2][i + 1], rings[j][i + 1]);
            mesh.push_triangle(a, b, c);
            mesh.push_triangle(a, c, d);
        }
    }
    for j in 0..n as usize {
        let j2 = (j + 1) % n as usize;
        let center = crate::vec3::Vec3::new(0.0, 0.0, rings[0][0].z);
        mesh.push_triangle(center, rings[j2][0], rings[j][0]);
    }
    mesh
}

pub fn capsule_mesh(diameter: f64, total_length: f64, segments: u32) -> Mesh {
    let r = diameter / 2.0;
    let cyl_h = (total_length - diameter).max(0.0);
    let half_cyl = cyl_h / 2.0;
    let lat = ((segments / 4).max(2)) as i64;
    let mut profile = Vec::new();
    for i in 0..=lat {
        let phi = -FRAC_PI_2 + FRAC_PI_2 * (i as f64 / lat as f64);
        profile.push((r * phi.cos(), -half_cyl + r * phi.sin()));
    }
    for i in 0..=lat {
        let phi = FRAC_PI_2 * (i as f64 / lat as f64);
        profile.push((r * phi.cos(), half_cyl + r * phi.sin()));
    }
    revolve_open_profile(&profile, segments.max(3), false, false)
}

pub fn tetrahedron_mesh(size: f64, as_edge_length: bool) -> Mesh {
    let mode = if as_edge_length { SizeMode::EdgeLength } else { SizeMode::CircumscribedDiameter };
    polyhedra::tetrahedron_mesh(size, mode)
}

pub fn octahedron_mesh(size: f64, as_edge_length: bool) -> Mesh {
    let mode = if as_edge_length { SizeMode::EdgeLength } else { SizeMode::CircumscribedDiameter };
    polyhedra::octahedron_mesh(size, mode)
}

pub fn dodecahedron_mesh(size: f64, as_edge_length: bool) -> Mesh {
    let mode = if as_edge_length { SizeMode::EdgeLength } else { SizeMode::CircumscribedDiameter };
    polyhedra::dodecahedron_mesh(size, mode)
}

pub fn icosahedron_mesh(size: f64, as_edge_length: bool) -> Mesh {
    let mode = if as_edge_length { SizeMode::EdgeLength } else { SizeMode::CircumscribedDiameter };
    polyhedra::icosahedron_mesh(size, mode)
}
