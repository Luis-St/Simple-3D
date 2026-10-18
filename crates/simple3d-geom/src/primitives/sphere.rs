//! Ellipsoids and what is cut or built from them.

use crate::mesh::Mesh;
use crate::revolve::{grid_mesh, revolve_open_profile};
use crate::vec3::Vec3;
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

/// An ellipsoid hollowed by a `wall` (its inside one wall smaller on every axis), or cut to its upper
/// half, or both: a dome. Neither gives exactly [`ellipsoid_mesh`].
pub fn ellipsoid_shell_mesh(dx: f64, dy: f64, dz: f64, wall: f64, upper_half: bool, segments: u32) -> Mesh {
    if wall <= 0.0 && !upper_half {
        return ellipsoid_mesh(dx, dy, dz, segments);
    }
    let outer = [dx / 2.0, dy / 2.0, dz / 2.0];
    let inner = outer.map(|r| r - wall.max(0.0));
    let lat = if upper_half { (segments / 4).max(2) } else { (segments / 2).max(2) };
    let cut = upper_half.then_some(0.0);
    let mesh = shell(outer, (wall > 0.0).then_some(inner), cut, segments.max(3), lat);
    // Centred on its own height, as every generator is.
    if upper_half {
        mesh.translated(Vec3::new(0.0, 0.0, -outer[2] / 2.0))
    } else {
        mesh
    }
}

/// The part of a sphere `diameter` across above a plane, `cap_height` tall, hollowed by a `wall`. The
/// height is held to the diameter, the whole sphere.
pub fn cap_shell_mesh(diameter: f64, cap_height: f64, wall: f64, segments: u32) -> Mesh {
    let r = diameter / 2.0;
    let height = cap_height.clamp(1e-6, diameter);
    let inner = r - wall.max(0.0);
    let lat = (segments / 2).max(2);
    let mesh = shell([r; 3], (wall > 0.0).then_some([inner; 3]), Some(r - height), segments.max(3), lat);
    mesh.translated(Vec3::new(0.0, 0.0, height / 2.0 - r))
}

/// The sphere a cap of `base` diameter and `height` is cut from, as a diameter: any pair has one.
pub fn cap_sphere_diameter(base: f64, height: f64) -> f64 {
    let (a, h) = (base / 2.0, height.max(1e-9));
    (a * a + h * h) / h
}

/// An ellipsoid of semi-axes `outer` from a plane at `cut` (above its middle) or its bottom pole to its
/// top pole, and back down inside at `inner` if hollow, as rings of `n` points about Z. Where the
/// inside does not reach the plane, the cut face is whole.
fn shell(outer: [f64; 3], inner: Option<[f64; 3]>, cut: Option<f64>, n: u32, lat: u32) -> Mesh {
    let from = |radii: [f64; 3]| cut.map_or(-FRAC_PI_2, |z| (z / radii[2]).clamp(-1.0, 1.0).asin());
    let ring = |radii: [f64; 3], phi: f64| -> Vec<Vec3> {
        (0..n)
            .map(|j| {
                let theta = TAU * j as f64 / n as f64;
                Vec3::new(radii[0] * phi.cos() * theta.cos(), radii[1] * phi.cos() * theta.sin(), radii[2] * phi.sin())
            })
            .collect()
    };
    let arc = |radii: [f64; 3]| -> Vec<Vec<Vec3>> {
        let start = from(radii);
        (0..=lat).map(|i| ring(radii, start + (FRAC_PI_2 - start) * i as f64 / lat as f64)).collect()
    };
    let mut rings = arc(outer);
    let inner = inner.filter(|r| r.iter().all(|&v| v > 1e-9) && cut.is_none_or(|z| z < r[2]));
    match (inner, cut) {
        (Some(radii), _) => rings.extend(arc(radii).into_iter().rev()),
        (None, Some(z)) => rings.push(vec![Vec3::new(0.0, 0.0, z); n as usize]),
        // A whole solid ellipsoid runs pole to pole; the poles' rings are single points.
        (None, None) => {}
    }
    grid_mesh(&rings, true)
}
