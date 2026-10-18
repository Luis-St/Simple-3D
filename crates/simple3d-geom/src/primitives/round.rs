//! Solids of revolution: prisms, cones, cylinders, discs, tubes, rings and the torus.

use super::*;
use crate::mesh::Mesh;
use crate::revolve::{extrude_frustum_polygon, grid_mesh, revolve_closed_profile, ring_outline, sector_outline};
use crate::vec3::Vec3;
use std::f64::consts::{PI, TAU};

pub fn regular_prism_mesh(sides: u32, diameter: f64, height: f64, across_flats: bool) -> Mesh {
    let r = polygon_radius(sides.max(3), diameter, across_flats);
    let outline = ring_outline(sides.max(3), r, r);
    extrude_frustum_polygon(&outline, &outline, height)
}

/// A regular prism turned `twist_deg` from bottom to top, hollowed by a `wall` measured square to the
/// flats; a wall reaching the middle leaves it solid. Neither gives exactly [`regular_prism_mesh`].
pub fn twisted_prism_mesh(
    sides: u32,
    diameter: f64,
    height: f64,
    across_flats: bool,
    twist_deg: f64,
    wall: f64,
) -> Mesh {
    let n = sides.max(3);
    let r = polygon_radius(n, diameter, across_flats);
    let inner = r - wall.max(0.0) / (PI / n as f64).cos();
    let hollow = wall > 1e-9 && inner > 1e-9;
    if twist_deg.abs() < 1e-9 && !hollow {
        return regular_prism_mesh(sides, diameter, height, across_flats);
    }
    // A level every few degrees, so the twisted faces stay close to the surface they stand for.
    let levels = ((twist_deg.abs() / 3.0).ceil() as usize).max(1);
    let level = |radius: f64, k: usize| -> Vec<Vec3> {
        let t = k as f64 / levels as f64;
        let turn = twist_deg.to_radians() * t;
        let z = -height / 2.0 + height * t;
        (0..n)
            .map(|i| {
                let a = TAU * i as f64 / n as f64 + turn;
                Vec3::new(radius * a.cos(), radius * a.sin(), z)
            })
            .collect()
    };
    let mut rings: Vec<Vec<Vec3>> = (0..=levels).map(|k| level(r, k)).collect();
    if !hollow {
        return grid_mesh(&rings, false);
    }
    rings.extend((0..=levels).rev().map(|k| level(inner, k)));
    grid_mesh(&rings, true)
}

pub fn cone_mesh(bottom_diameter: f64, top_diameter: f64, height: f64, segments: u32) -> Mesh {
    cone_sector_mesh(bottom_diameter, top_diameter, height, segments, 360.0)
}

/// A cone over part of a turn, closed by its two cut faces.
pub fn cone_sector_mesh(bottom_diameter: f64, top_diameter: f64, height: f64, segments: u32, sweep_deg: f64) -> Mesh {
    let n = segments.max(3);
    let bottom = sector_outline(n, bottom_diameter / 2.0, bottom_diameter / 2.0, sweep_deg);
    let top = sector_outline(n, top_diameter / 2.0, top_diameter / 2.0, sweep_deg);
    extrude_frustum_polygon(&bottom, &top, height)
}

/// A cone whose wall is `wall` thick, measured square to its side, open at both ends; where the inside
/// reaches the axis it closes there, as a pointed cone's tip. A wall of 0 gives [`cone_sector_mesh`].
pub fn hollow_cone_sector_mesh(
    bottom_diameter: f64,
    top_diameter: f64,
    height: f64,
    wall: f64,
    segments: u32,
    sweep_deg: f64,
) -> Mesh {
    let (rb, rt, hh) = (bottom_diameter / 2.0, top_diameter / 2.0, height / 2.0);
    // The wall across, level by level, is wider than square to the side by the slope's secant.
    let slope = (rt - rb) / height.max(1e-12);
    let across = wall.max(0.0) * (1.0 + slope * slope).sqrt();
    let (ib, it) = (rb - across, rt - across);
    if wall <= 0.0 || (ib <= 1e-9 && it <= 1e-9) {
        return cone_sector_mesh(bottom_diameter, top_diameter, height, segments, sweep_deg);
    }
    // Where the inside meets the axis, as a share of the height from the bottom.
    let meets = |from: f64, to: f64| -hh + height * from / (from - to);
    let profile = if it <= 1e-9 {
        vec![(ib, -hh), (rb, -hh), (rt, hh), (0.0, hh), (0.0, meets(ib, it))]
    } else if ib <= 1e-9 {
        vec![(0.0, meets(ib, it)), (0.0, -hh), (rb, -hh), (rt, hh), (it, hh)]
    } else {
        vec![(ib, -hh), (rb, -hh), (rt, hh), (it, hh)]
    };
    revolve_closed_profile(&profile, segments.max(3), sweep_deg)
}

/// Lean a shape `height` tall so its top moves by (`dx`, `dy`) and its bottom stays: a pyramid's apex
/// moved off the middle. Flat faces stay flat.
pub fn skewed(mesh: Mesh, height: f64, dx: f64, dy: f64) -> Mesh {
    if dx == 0.0 && dy == 0.0 {
        return mesh;
    }
    let mut mesh = mesh;
    for p in mesh.positions.iter_mut() {
        let t = (p.z + height / 2.0) / height.max(1e-12);
        p.x += dx * t;
        p.y += dy * t;
    }
    mesh
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

/// A cylinder over part of a turn: a capped pie slice.
pub fn cylinder_sector_mesh(dx: f64, dy: f64, height: f64, segments: u32, sweep_deg: f64) -> Mesh {
    let outline = sector_outline(segments.max(3), dx / 2.0, dy / 2.0, sweep_deg);
    extrude_frustum_polygon(&outline, &outline, height)
}

/// [`cylinder_sector_mesh`] whose top is `top_scale` times the bottom's size: tapered, or flared above 1.
pub fn tapered_cylinder_sector_mesh(
    dx: f64,
    dy: f64,
    height: f64,
    top_scale: f64,
    segments: u32,
    sweep_deg: f64,
) -> Mesh {
    let bottom = sector_outline(segments.max(3), dx / 2.0, dy / 2.0, sweep_deg);
    let s = top_scale.max(0.0);
    let top = sector_outline(segments.max(3), s * dx / 2.0, s * dy / 2.0, sweep_deg);
    extrude_frustum_polygon(&bottom, &top, height)
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

/// A tube over part of a turn (a channel or elbow), closed by its cut faces.
pub fn tube_sector_mesh(outer_diameter: f64, inner_diameter: f64, height: f64, segments: u32, sweep_deg: f64) -> Mesh {
    let ri = inner_diameter / 2.0;
    let ro = outer_diameter / 2.0;
    let profile = [(ri, -height / 2.0), (ro, -height / 2.0), (ro, height / 2.0), (ri, height / 2.0)];
    revolve_closed_profile(&profile, segments.max(3), sweep_deg)
}

/// A tube with diameters of its own at the top; each inner one is held to its outer one.
pub fn tapered_tube_sector_mesh(outer: [f64; 2], inner: [f64; 2], height: f64, segments: u32, sweep_deg: f64) -> Mesh {
    let (rb, rt) = (outer[0] / 2.0, outer[1] / 2.0);
    let (ib, it) = ((inner[0] / 2.0).clamp(0.0, rb), (inner[1] / 2.0).clamp(0.0, rt));
    let profile = [(ib, -height / 2.0), (rb, -height / 2.0), (rt, height / 2.0), (it, height / 2.0)];
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
