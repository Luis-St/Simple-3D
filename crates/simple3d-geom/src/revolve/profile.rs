//! Revolving a profile about the Y axis.

use crate::mesh::Mesh;
use crate::vec3::Vec3;
use std::f64::consts::TAU;

/// Revolve an open (radius, z) profile curve fully around Z. Ends whose
/// radius is ~0 close naturally at the axis; ends with radius > 0 are left
/// open unless `cap_start` / `cap_end` request a flat disc there.
pub fn revolve_open_profile(profile: &[(f64, f64)], segments: u32, cap_start: bool, cap_end: bool) -> Mesh {
    let n = segments.max(3);
    let m = profile.len();
    let mut mesh = Mesh::new();
    let rings: Vec<Vec<Vec3>> = (0..n)
        .map(|j| {
            let theta = TAU * j as f64 / n as f64;
            profile.iter().map(|&(r, z)| Vec3::new(r * theta.cos(), r * theta.sin(), z)).collect()
        })
        .collect();
    for j in 0..n as usize {
        let j2 = (j + 1) % n as usize;
        for i in 0..m - 1 {
            let a = rings[j][i];
            let b = rings[j2][i];
            let c = rings[j2][i + 1];
            let d = rings[j][i + 1];
            mesh.push_triangle(a, b, c);
            mesh.push_triangle(a, c, d);
        }
    }
    if cap_start && profile[0].0 > 1e-9 {
        let center = Vec3::new(0.0, 0.0, profile[0].1);
        for j in 0..n as usize {
            let j2 = (j + 1) % n as usize;
            mesh.push_triangle(center, rings[j2][0], rings[j][0]);
        }
    }
    if cap_end && profile[m - 1].0 > 1e-9 {
        let center = Vec3::new(0.0, 0.0, profile[m - 1].1);
        for j in 0..n as usize {
            let j2 = (j + 1) % n as usize;
            mesh.push_triangle(center, rings[j][m - 1], rings[j2][m - 1]);
        }
    }
    mesh
}

/// Revolve a *closed* (radius, z) loop (traversed CCW in the (r,z) plane,
/// e.g. an annulus rectangle or a tube's circular cross-section) around Z.
/// If `sweep_deg` is short of 360, the two open ends are fan-capped.
pub fn revolve_closed_profile(profile: &[(f64, f64)], segments: u32, sweep_deg: f64) -> Mesh {
    let n = segments.max(3);
    let sweep = sweep_deg.to_radians().clamp(0.0, TAU);
    let full = (TAU - sweep).abs() < 1e-9;
    let ring_count = if full { n } else { n + 1 };
    let m = profile.len();
    let mut mesh = Mesh::new();
    let rings: Vec<Vec<Vec3>> = (0..ring_count)
        .map(|j| {
            let theta = if full { TAU * j as f64 / n as f64 } else { sweep * j as f64 / n as f64 };
            profile.iter().map(|&(r, z)| Vec3::new(r * theta.cos(), r * theta.sin(), z)).collect()
        })
        .collect();
    let steps = if full { ring_count } else { ring_count - 1 };
    for j in 0..steps as usize {
        let j2 = if full { (j + 1) % ring_count as usize } else { j + 1 };
        for i in 0..m {
            let i2 = (i + 1) % m;
            let a = rings[j][i];
            let b = rings[j2][i];
            let c = rings[j2][i2];
            let d = rings[j][i2];
            mesh.push_triangle(a, b, c);
            mesh.push_triangle(a, c, d);
        }
    }
    if !full {
        let centroid =
            |ring: &[Vec3]| -> Vec3 { ring.iter().fold(Vec3::ZERO, |a, &b| a + b) * (1.0 / ring.len() as f64) };
        let start = &rings[0];
        let c0 = centroid(start);
        for i in 0..m {
            let i2 = (i + 1) % m;
            mesh.push_triangle(c0, start[i], start[i2]);
        }
        let end = &rings[ring_count as usize - 1];
        let c1 = centroid(end);
        for i in 0..m {
            let i2 = (i + 1) % m;
            mesh.push_triangle(c1, end[i2], end[i]);
        }
    }
    mesh
}
