//! Shapes wound about Z (issue 123): a coil spring and a threaded rod.

use crate::mesh::Mesh;
use crate::vec3::Vec3;
use std::f64::consts::TAU;

/// Rows of a threaded rod per pitch: enough that a crest's flat lands within an eighth of its depth.
const THREAD_ROWS: f64 = 12.0;

/// The most rows a threaded rod is drawn with, however long and fine.
const MOST_THREAD_ROWS: usize = 4000;

/// A coil spring `diameter` across outside, of round wire `wire` thick, `turns` turns of `pitch`
/// each, centred on the origin and starting on +X. The wire is narrowed to fit the coil, and the
/// pitch widened to keep the turns apart.
pub fn spring_mesh(diameter: f64, wire: f64, pitch: f64, turns: f64, segments: u32) -> Mesh {
    let wire = wire.min(diameter * 0.9) / 2.0;
    let coil = diameter / 2.0 - wire;
    let pitch = pitch.max(wire * 2.2);
    let per_turn = segments.max(8) as usize;
    let steps = ((turns.max(0.05) * per_turn as f64).round() as usize).max(1);
    let around = (segments as usize / 2).max(8);
    let rise = pitch * steps as f64 / per_turn as f64;
    // The wire's section lies in the plane through the axis, as for a gently pitched coil it nearly is.
    let rings: Vec<Vec<Vec3>> = (0..=steps)
        .map(|j| {
            let theta = TAU * j as f64 / per_turn as f64;
            let z = pitch * theta / TAU - rise / 2.0;
            (0..around)
                .map(|k| {
                    let phi = TAU * k as f64 / around as f64;
                    let r = coil + wire * phi.cos();
                    Vec3::new(r * theta.cos(), r * theta.sin(), z + wire * phi.sin())
                })
                .collect()
        })
        .collect();
    let mut mesh = Mesh::new();
    for pair in rings.windows(2) {
        for k in 0..around {
            let l = (k + 1) % around;
            mesh.push_triangle(pair[0][k], pair[1][k], pair[1][l]);
            mesh.push_triangle(pair[0][k], pair[1][l], pair[0][l]);
        }
    }
    for (ring, flip) in [(&rings[0], true), (&rings[steps], false)] {
        let centre = ring.iter().fold(Vec3::ZERO, |sum, &p| sum + p) * (1.0 / around as f64);
        for k in 0..around {
            let (a, b) = (ring[k], ring[(k + 1) % around]);
            if flip {
                mesh.push_triangle(centre, a, b)
            } else {
                mesh.push_triangle(centre, b, a)
            }
        }
    }
    outward(mesh)
}

/// A rod `diameter` across its crests with a metric (ISO 60 degree) thread of `pitch`, `length`
/// long and centred on the origin. To cut a matching inner thread, give a slightly larger one.
pub fn threaded_rod_mesh(diameter: f64, pitch: f64, length: f64, segments: u32) -> Mesh {
    let major = diameter / 2.0;
    let pitch = pitch.max(1e-3);
    // The basic profile's depth, five eighths of the sharp triangle's.
    let minor = (major - 0.625 * 0.866_025 * pitch).max(major * 0.1);
    let radius = |theta: f64, z: f64| {
        let u = ((z - pitch * theta / TAU) / pitch).rem_euclid(1.0);
        let (crest, fall, root) = (1.0 / 8.0, 5.0 / 16.0, 1.0 / 4.0);
        let depth = match u {
            u if u < crest => 0.0,
            u if u < crest + fall => (u - crest) / fall,
            u if u < crest + fall + root => 1.0,
            u => 1.0 - (u - crest - fall - root) / fall,
        };
        major - (major - minor) * depth
    };
    let columns = segments.max(8) as usize;
    let rows = ((length / pitch * THREAD_ROWS).ceil() as usize).clamp(2, MOST_THREAD_ROWS);
    let grid: Vec<Vec<Vec3>> = (0..=rows)
        .map(|i| {
            let z = -length / 2.0 + length * i as f64 / rows as f64;
            (0..columns)
                .map(|k| {
                    let theta = TAU * k as f64 / columns as f64;
                    let r = radius(theta, z);
                    Vec3::new(r * theta.cos(), r * theta.sin(), z)
                })
                .collect()
        })
        .collect();
    let mut mesh = Mesh::new();
    for pair in grid.windows(2) {
        for k in 0..columns {
            let l = (k + 1) % columns;
            mesh.push_triangle(pair[0][k], pair[0][l], pair[1][l]);
            mesh.push_triangle(pair[0][k], pair[1][l], pair[1][k]);
        }
    }
    for (ring, z, up) in [(&grid[0], -length / 2.0, false), (&grid[rows], length / 2.0, true)] {
        let centre = Vec3::new(0.0, 0.0, z);
        for k in 0..columns {
            let (a, b) = (ring[k], ring[(k + 1) % columns]);
            if up {
                mesh.push_triangle(centre, a, b)
            } else {
                mesh.push_triangle(centre, b, a)
            }
        }
    }
    outward(mesh)
}

/// The mesh turned outside out if it was built inside out.
fn outward(mut mesh: Mesh) -> Mesh {
    if mesh.signed_volume() < 0.0 {
        mesh.flip_winding();
    }
    mesh
}
