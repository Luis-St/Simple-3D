//! The plane a grid is drawn on: the ground, or a workplane (issue 125).
//!
//! Everything about the grid's extent is worked out in the plane's own two coordinates, so a
//! workplane's grid fades, steps decades and snaps exactly as the ground's does.

use crate::view::View;
use simple3d_geom::Vec3;

/// Lines per major line on the ground and workplanes.
pub const DECADE: u32 = 10;

/// A grid's plane and spacing: its centre and the unit directions its lines run along.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridPlane {
    pub origin: Vec3,
    pub u: Vec3,
    pub v: Vec3,
    /// Line spacing in millimetres.
    pub spacing: f64,
    /// How many lines of a level make one major line, and the factor from one level to the next:
    /// ten for the ground and workplanes, the document grid's spacing in millimetres while sketching
    /// on a 1 mm grid (issue 74). Never below two, so the levels always grow.
    pub major_every: u32,
    /// Whether the lines a `major_every` apart are drawn heavier over the finer ones: the document
    /// grid's lines on the 1 mm sketch grid (issue 74).
    pub emphasise: bool,
}

impl GridPlane {
    /// The ground: the XY plane through the world origin.
    pub fn ground(spacing: f64) -> GridPlane {
        GridPlane {
            origin: Vec3::ZERO,
            u: Vec3::new(1.0, 0.0, 0.0),
            v: Vec3::new(0.0, 1.0, 0.0),
            spacing,
            major_every: DECADE,
            emphasise: false,
        }
    }

    /// A workplane's grid.
    pub fn of(plane: &simple3d_core::scene::Workplane) -> GridPlane {
        let [u, v, _] = plane.axes();
        GridPlane { origin: plane.origin(), u, v, spacing: plane.sane_spacing(), major_every: DECADE, emphasise: false }
    }

    /// [`GridPlane::major_every`] as a factor, kept at two or more.
    pub fn every(&self) -> f64 {
        self.major_every.max(2) as f64
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

    /// How heavy the emphasised lines are drawn for the levels `grid_levels_on` gave, from 0 (not at
    /// all) to 1, and whether every coarse line is one of them rather than only its majors. Full
    /// while the plane's own spacing is the coarse level, fading with the fine level when that is
    /// the plane's spacing, since the heavier lines only stand out against finer ones. Full already
    /// at a quarter of the fine level's strength, so they read as soon as the fine lines do.
    pub fn bold(&self, fine: f64, coarse: f64, strength: f64) -> (f64, bool) {
        let own = self.spacing * 1.000_001;
        match () {
            _ if !self.emphasise => (0.0, false),
            _ if coarse <= own => (1.0, false),
            _ if fine <= own => ((strength * 4.0).min(1.0), true),
            _ => (0.0, false),
        }
    }

    /// The hash of everything that moves the drawn grid.
    pub fn hash_bits<H: std::hash::Hasher>(&self, hasher: &mut H) {
        use std::hash::Hash;
        for p in [self.origin, self.u, self.v] {
            [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()].hash(hasher);
        }
        self.spacing.to_bits().hash(hasher);
        self.major_every.hash(hasher);
        self.emphasise.hash(hasher);
    }
}
