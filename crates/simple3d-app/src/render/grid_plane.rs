//! The plane a grid is drawn on: the ground, or a workplane (issue 125).
//!
//! Everything about the grid's extent is worked out in the plane's own two coordinates, so a
//! workplane's grid fades, steps decades and snaps exactly as the ground's does.

use crate::view::View;
use simple3d_geom::Vec3;

/// A grid's plane and spacing: its centre and the unit directions its lines run along.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridPlane {
    pub origin: Vec3,
    pub u: Vec3,
    pub v: Vec3,
    /// Line spacing in millimetres.
    pub spacing: f64,
}

impl GridPlane {
    /// The ground: the XY plane through the world origin.
    pub fn ground(spacing: f64) -> GridPlane {
        GridPlane { origin: Vec3::ZERO, u: Vec3::new(1.0, 0.0, 0.0), v: Vec3::new(0.0, 1.0, 0.0), spacing }
    }

    /// A workplane's grid.
    pub fn of(plane: &simple3d_core::scene::Workplane) -> GridPlane {
        let [u, v, _] = plane.axes();
        GridPlane { origin: plane.origin(), u, v, spacing: plane.sane_spacing() }
    }

    pub fn normal(&self) -> Vec3 {
        self.u.cross(self.v)
    }

    /// The world point at `s` along `u` and `t` along `v`.
    pub fn at(&self, s: f64, t: f64) -> Vec3 {
        self.origin + self.u * s + self.v * t
    }

    /// `p`'s coordinates in the plane, dropping its height off it.
    pub fn local(&self, p: Vec3) -> (f64, f64) {
        let d = p - self.origin;
        (d.dot(self.u), d.dot(self.v))
    }

    /// The point on the plane under the pointer, in front of the eye.
    pub fn under(&self, view: &View, cursor: egui::Pos2) -> Option<Vec3> {
        view.ray_plane_ahead(cursor, self.origin, self.normal())
    }

    /// `p` moved onto the plane, rounded to `step` along its two directions.
    pub fn round(&self, p: Vec3, step: f64) -> Vec3 {
        let (s, t) = self.local(p);
        if step <= 0.0 {
            return self.at(s, t);
        }
        self.at((s / step).round() * step, (t / step).round() * step)
    }

    /// The hash of everything that moves the drawn grid.
    pub fn hash_bits<H: std::hash::Hasher>(&self, hasher: &mut H) {
        use std::hash::Hash;
        for p in [self.origin, self.u, self.v] {
            [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()].hash(hasher);
        }
        self.spacing.to_bits().hash(hasher);
    }
}
