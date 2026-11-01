//! An ellipse as plane geometry: a centre, the unit direction of its major axis and its two
//! half-axes, with the angle along it that sampling, the solver and the region graph all go by.

use crate::{cross, dot, sub};
use std::f64::consts::TAU;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ellipse {
    pub centre: [f64; 2],
    /// The major axis as a unit direction.
    pub axis: [f64; 2],
    /// The half-axis along `axis`, from the centre to the major point.
    pub a: f64,
    /// The other half-axis.
    pub b: f64,
}

impl Ellipse {
    /// The ellipse about `centre` whose `axis` half-axis ends on `major`, with `minor` the other.
    pub fn new(centre: [f64; 2], major: [f64; 2], minor: f64) -> Ellipse {
        let d = sub(major, centre);
        let a = crate::length(d);
        let axis = if a > 1e-12 { [d[0] / a, d[1] / a] } else { [1.0, 0.0] };
        Ellipse { centre, axis, a, b: minor.abs() }
    }

    /// Point `p` in the ellipse's own frame: along the axis, then a quarter turn left of it.
    fn local(&self, p: [f64; 2]) -> [f64; 2] {
        let q = sub(p, self.centre);
        [dot(q, self.axis), cross(self.axis, q)]
    }

    /// The angle along the ellipse of the place nearest `p` in its own scaled frame, in (-pi, pi].
    pub fn angle_of(&self, p: [f64; 2]) -> f64 {
        let [u, v] = self.local(p);
        (v / self.b.max(1e-12)).atan2(u / self.a.max(1e-12))
    }

    pub fn at(&self, t: f64) -> [f64; 2] {
        let (u, v) = (self.a * t.cos(), self.b * t.sin());
        let n = [-self.axis[1], self.axis[0]];
        [self.centre[0] + u * self.axis[0] + v * n[0], self.centre[1] + u * self.axis[1] + v * n[1]]
    }

    /// Which way the ellipse runs at angle `t`, counter-clockwise.
    pub fn tangent(&self, t: f64) -> [f64; 2] {
        let (du, dv) = (-self.a * t.sin(), self.b * t.cos());
        let n = [-self.axis[1], self.axis[0]];
        [du * self.axis[0] + dv * n[0], du * self.axis[1] + dv * n[1]]
    }

    /// How far `p` is off the ellipse, roughly in millimetres: zero on it, below zero inside.
    pub fn residual(&self, p: [f64; 2]) -> f64 {
        let [u, v] = self.local(p);
        let (a, b) = (self.a.max(1e-12), self.b.max(1e-12));
        ((u / a).hypot(v / b) - 1.0) * a.min(b)
    }

    /// The minor half-axis making the ellipse about `centre` with major point `major` run through
    /// `p`; `None` when `p` is not between the ends of the major axis.
    pub fn minor_through(centre: [f64; 2], major: [f64; 2], p: [f64; 2]) -> Option<f64> {
        let e = Ellipse::new(centre, major, 1.0);
        let [u, v] = e.local(p);
        let along = u / e.a.max(1e-12);
        (along.abs() < 1.0 - 1e-6 && v.abs() > 1e-9).then(|| v.abs() / (1.0 - along * along).sqrt())
    }
}

/// The angle an arc of `e` from `s` to `end` starts at and its counter-clockwise sweep in (0, 2 pi].
pub(crate) fn arc_span(e: &Ellipse, s: [f64; 2], end: [f64; 2]) -> (f64, f64) {
    let t0 = e.angle_of(s);
    let mut sweep = (e.angle_of(end) - t0).rem_euclid(TAU);
    if sweep < 1e-9 {
        sweep = TAU;
    }
    (t0, sweep)
}
