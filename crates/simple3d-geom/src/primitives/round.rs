//! The solids of revolution: prisms, cones, cylinders, discs, tubes, rings
//! and the torus.

use super::*;
use crate::mesh::Mesh;
use crate::revolve::{extrude_frustum_polygon, revolve_closed_profile, ring_outline, sector_outline};
use std::f64::consts::TAU;

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
pub fn cone_sector_mesh(bottom_diameter: f64, top_diameter: f64, height: f64, segments: u32, sweep_deg: f64) -> Mesh {
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
pub fn tube_sector_mesh(outer_diameter: f64, inner_diameter: f64, height: f64, segments: u32, sweep_deg: f64) -> Mesh {
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
