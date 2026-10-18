//! A coil spring wound about Z (issue 123).

use crate::mesh::Mesh;
use crate::vec3::Vec3;
use std::f64::consts::TAU;

/// How a spring's ends are finished.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpringEnds {
    Open,
    /// A turn at each end rising only by the wire's thickness, closing onto the next.
    Closed,
    /// Closed, then cut flat through the middle of the wire at each end, so it stands upright.
    Ground,
}

impl SpringEnds {
    /// From the registry's choice index, in declaration order.
    pub fn from_index(index: u32) -> SpringEnds {
        match index {
            1 => SpringEnds::Closed,
            2 => SpringEnds::Ground,
            _ => SpringEnds::Open,
        }
    }
}

/// A coil spring `diameter` across outside, of round wire `wire` thick, `turns` turns of `pitch`
/// each, centred on the origin and starting on +X. The wire is narrowed to fit the coil.
pub fn spring_mesh(diameter: f64, wire: f64, pitch: f64, turns: f64, segments: u32) -> Mesh {
    coil_spring_mesh(diameter, wire, pitch, turns, SpringEnds::Open, segments)
}

/// The pitch that makes a spring `height` tall overall; negative when even no pitch is short enough.
pub fn spring_pitch_for_height(height: f64, diameter: f64, wire: f64, turns: f64, ends: SpringEnds) -> f64 {
    let wire = wire.min(diameter * 0.9);
    let fixed = match ends {
        SpringEnds::Open => wire,
        SpringEnds::Closed => 3.0 * wire,
        SpringEnds::Ground => 2.0 * wire,
    };
    (height - fixed) / turns.max(0.05)
}

/// [`spring_mesh`] with its ends finished as `ends`, the closing turns added to the `turns` of `pitch`.
/// Any pitch is taken, down to 0; below the wire's thickness the turns run into one another.
pub fn coil_spring_mesh(diameter: f64, wire: f64, pitch: f64, turns: f64, ends: SpringEnds, segments: u32) -> Mesh {
    let wire = wire.min(diameter * 0.9) / 2.0;
    let coil = diameter / 2.0 - wire;
    let pitch = pitch.max(0.0);
    let turns = turns.max(0.05);
    let closing = if ends == SpringEnds::Open { 0.0 } else { 1.0 };
    let total = turns + 2.0 * closing;
    // The middle of the wire's height after `turn` turns: closing turns rise by the wire's thickness.
    let rise = |turn: f64| -> f64 {
        let first = turn.min(closing);
        let active = (turn - closing).clamp(0.0, turns);
        let last = (turn - closing - turns).max(0.0);
        2.0 * wire * (first + last) + pitch * active
    };
    let per_turn = segments.max(8) as usize;
    let around = (segments as usize / 2).max(8);
    // Whole steps, so a ring lands on +X and -X for the width, then the part turn's end if any.
    let whole = (total * per_turn as f64 + 1e-9).floor() as usize;
    let mut steps: Vec<f64> = (0..=whole).map(|j| j as f64 / per_turn as f64).collect();
    if total - steps[whole] > 1e-9 {
        steps.push(total);
    }
    let half = rise(total) / 2.0;
    let flat = |z: f64| if ends == SpringEnds::Ground { z.clamp(-half, half) } else { z };
    // The wire's section lies in the plane through the axis, as for a gently pitched coil it nearly is.
    let rings: Vec<Vec<Vec3>> = steps
        .iter()
        .map(|&turn| {
            let theta = TAU * turn;
            let z = rise(turn) - half;
            (0..around)
                .map(|k| {
                    let phi = TAU * k as f64 / around as f64;
                    let r = coil + wire * phi.cos();
                    Vec3::new(r * theta.cos(), r * theta.sin(), flat(z + wire * phi.sin()))
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
    let last = rings.len() - 1;
    for (ring, flip) in [(&rings[0], true), (&rings[last], false)] {
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
    mesh.turned_outward()
}
