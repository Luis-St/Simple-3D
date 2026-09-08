//! Ellipsoids and what is cut or built from them.

use crate::mesh::Mesh;
use crate::revolve::revolve_open_profile;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

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
