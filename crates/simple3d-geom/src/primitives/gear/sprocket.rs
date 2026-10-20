//! A roller chain sprocket (issue 123) with the ISO 606 tooth form: a seat round each roller, flanks
//! of the standard radius rising from it, and the tips cut to the middle of the standard's range.

use super::Body;
use crate::mesh::Mesh;
use std::f64::consts::{PI, TAU};

/// Points along a roller's seat, and along a flank.
const SEAT_POINTS: usize = 6;
const FLANK_POINTS: usize = 8;

/// Everything a sprocket is made of.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SprocketSpec {
    pub teeth: u32,
    /// The chain's pitch, from one roller to the next.
    pub pitch: f64,
    pub roller: f64,
    /// The wheel round the teeth; its twist plays no part.
    pub body: Body,
}

impl SprocketSpec {
    fn count(&self) -> f64 {
        self.teeth.max(6) as f64
    }

    /// The circle the rollers' centres sit on.
    fn pitch_radius(&self) -> f64 {
        self.pitch / (2.0 * (PI / self.count()).sin())
    }

    /// The roller, held to where it leaves room for a tooth between two.
    fn roller_built(&self) -> f64 {
        self.roller.clamp(self.pitch * 0.1, self.pitch * 0.8)
    }

    /// The bottom of a roller's seat.
    pub fn root_radius(&self) -> f64 {
        self.pitch_radius() - seat_radius(self.roller_built())
    }

    /// The tip circle: halfway between the smallest and largest ISO 606 allows.
    fn tip_circle(&self) -> f64 {
        let (z, p, d1) = (self.count(), self.pitch, self.roller_built());
        self.pitch_radius() + (p * (1.125 - 0.8 / z) - d1) / 2.0
    }

    /// The tips as built: the tip circle, or lower where the flanks meet before it.
    pub fn tip_radius(&self) -> f64 {
        self.flank()[self.flank().len() - 1].0
    }

    /// A tooth's trailing flank, from the bottom of the seat to the tip's middle, as radii and angles off
    /// the tooth's middle, the angles shrinking.
    fn flank(&self) -> Vec<(f64, f64)> {
        let z = self.count();
        let d1 = self.roller_built();
        let seat = seat_radius(d1);
        let flank = 0.008 * d1 * (z * z + 180.0);
        let half_angle = (140.0 - 90.0 / z).to_radians() / 2.0;
        let tip = self.tip_circle();
        // The roller beside the tooth on +X, its seat turning from straight in towards that tooth.
        let around = PI / z;
        let centre = (self.pitch_radius() * around.cos(), self.pitch_radius() * around.sin());
        let towards = |beta: f64| (-(around + beta).cos(), -(around + beta).sin());
        let polar = |p: (f64, f64)| (p.0.hypot(p.1), p.1.atan2(p.0));
        let mut out: Vec<(f64, f64)> = (0..=SEAT_POINTS)
            .map(|i| {
                let d = towards(half_angle * i as f64 / SEAT_POINTS as f64);
                polar((centre.0 + seat * d.0, centre.1 + seat * d.1))
            })
            .collect();
        // The flank's circle touches the seat's where it ends, on the far side, curving the other way.
        let end = towards(half_angle);
        let middle = (centre.0 + (seat + flank) * end.0, centre.1 + (seat + flank) * end.1);
        let on_flank = |tau: f64| {
            let (s, c) = tau.sin_cos();
            polar((middle.0 - flank * (end.0 * c + end.1 * s), middle.1 - flank * (end.1 * c - end.0 * s)))
        };
        // Turned until the flank reaches the tip circle or the tooth's middle, whichever comes first.
        let done = |tau: f64| {
            let (r, a) = on_flank(tau);
            r >= tip || a <= 0.0
        };
        let (mut low, mut high) = (0.0, PI / 2.0);
        for _ in 0..60 {
            let mid = (low + high) / 2.0;
            if done(mid) {
                high = mid;
            } else {
                low = mid;
            }
        }
        out.extend((1..=FLANK_POINTS).map(|i| on_flank(high * i as f64 / FLANK_POINTS as f64)));
        let (r, a) = out[out.len() - 1];
        if a > 1e-9 {
            // The flat across the tip, to the middle.
            out.extend((1..=3).map(|i| (r, a * (1.0 - i as f64 / 3.0))));
        }
        out.last_mut().expect("has points").1 = 0.0;
        out
    }

    /// The outline, counter-clockwise from the tooth on +X.
    fn outline(&self) -> Vec<(f64, f64)> {
        let flank = self.flank();
        let z = self.teeth.max(6);
        let polar = |r: f64, angle: f64| (r * angle.cos(), r * angle.sin());
        let mut out = Vec::new();
        let (n, z) = (flank.len(), z as f64);
        for tooth in 0..self.teeth.max(6) {
            let middle = TAU * tooth as f64 / z;
            let next = middle + TAU / z;
            // The tip's middle, down the trailing flank to the seat's bottom, then up the next tooth's
            // leading flank short of its tip.
            out.push(polar(flank[n - 1].0, middle));
            out.extend(flank[1..n - 1].iter().rev().map(|&(r, off)| polar(r, middle + off)));
            out.push(polar(flank[0].0, middle + PI / z));
            out.extend(flank[1..n - 1].iter().map(|&(r, off)| polar(r, next - off)));
        }
        out
    }
}

/// The seat's radius for a roller of diameter `d1`, ISO 606's smallest.
fn seat_radius(d1: f64) -> f64 {
    0.505 * d1 + 0.069 * d1.cbrt()
}

/// The sprocket `spec` describes, centred on its whole height with a tooth along +X.
pub fn sprocket_mesh(spec: &SprocketSpec) -> Mesh {
    let body = Body { twist: 0.0, herringbone: false, ..spec.body };
    body.mesh(spec.outline(), spec.root_radius())
}
