//! A threaded rod (issue 123), of any of the usual thread forms or one of its own.

use crate::mesh::Mesh;
use crate::vec3::Vec3;
use std::f64::consts::TAU;

/// Rows of a threaded rod per pitch: enough that a crest's flat lands within an eighth of its depth.
const THREAD_ROWS: f64 = 12.0;

/// The most rows a threaded rod is drawn with, however long and fine.
const MOST_THREAD_ROWS: usize = 4000;

/// A thread's form over one pitch: its depth, and the shares of the pitch its crest, each flank and
/// its root take, which add up to one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThreadProfile {
    pub depth: f64,
    pub crest: f64,
    pub flank: f64,
    pub root: f64,
}

impl ThreadProfile {
    /// The ISO metric basic profile: 60 degrees, five eighths of the sharp triangle deep.
    pub fn iso(pitch: f64) -> ThreadProfile {
        ThreadProfile { depth: 0.625 * 0.866_025 * pitch, crest: 1.0 / 8.0, flank: 5.0 / 16.0, root: 1.0 / 4.0 }
    }

    /// The trapezoidal (ISO 2904) form: 30 degrees, half a pitch deep, for lead screws.
    pub fn trapezoidal(pitch: f64) -> ThreadProfile {
        ThreadProfile::custom(pitch, 30.0, pitch / 2.0)
    }

    /// Square: upright flanks, half a pitch deep.
    pub fn square(pitch: f64) -> ThreadProfile {
        ThreadProfile { depth: pitch / 2.0, crest: 0.5, flank: 0.0, root: 0.5 }
    }

    /// Flanks `angle` degrees apart and `depth` deep, crest and root sharing what they leave. A depth the
    /// angle has no room for in one pitch is cut to where the flanks meet.
    pub fn custom(pitch: f64, angle: f64, depth: f64) -> ThreadProfile {
        let slope = (angle.clamp(0.0, 179.0) / 2.0).to_radians().tan();
        let pitch = pitch.max(1e-9);
        let depth = if slope > 0.0 { depth.max(0.0).min(pitch / 2.0 / slope) } else { depth.max(0.0) };
        let flank = (depth * slope / pitch).min(0.5);
        let rest = (1.0 - 2.0 * flank) / 2.0;
        ThreadProfile { depth, crest: rest, flank, root: rest }
    }

    /// How deep the thread is cut a share `u` of a pitch along, 0 at the crest and 1 at the root.
    fn depth_at(&self, u: f64) -> f64 {
        let flank = self.flank.max(1e-12);
        match u {
            u if u < self.crest => 0.0,
            u if u < self.crest + self.flank => (u - self.crest) / flank,
            u if u < self.crest + self.flank + self.root => 1.0,
            u => (1.0 - (u - self.crest - self.flank - self.root) / flank).max(0.0),
        }
    }
}

/// A rod `diameter` across its crests with a metric (ISO 60 degree) thread of `pitch`, `length`
/// long and centred on the origin. To cut a matching inner thread, give a slightly larger one.
pub fn threaded_rod_mesh(diameter: f64, pitch: f64, length: f64, segments: u32) -> Mesh {
    thread_mesh(diameter, pitch, length, ThreadProfile::iso(pitch), false, segments)
}

/// [`threaded_rod_mesh`] with a thread of `profile`, winding the other way if `left_hand`. The core is
/// kept to a tenth of the diameter, however deep the thread.
pub fn thread_mesh(
    diameter: f64,
    pitch: f64,
    length: f64,
    profile: ThreadProfile,
    left_hand: bool,
    segments: u32,
) -> Mesh {
    let major = diameter / 2.0;
    let pitch = pitch.max(1e-3);
    let minor = (major - profile.depth).max(major * 0.1);
    let hand = if left_hand { -1.0 } else { 1.0 };
    let radius = |theta: f64, z: f64| {
        let u = ((z - hand * pitch * theta / TAU) / pitch).rem_euclid(1.0);
        major - (major - minor) * profile.depth_at(u)
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
    mesh.turned_outward()
}
