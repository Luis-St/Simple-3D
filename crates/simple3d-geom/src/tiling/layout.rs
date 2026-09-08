//! Reading a tiling's own numbers: its axes, its step, and how many cells
//! and layers those come to.

use super::*;
use crate::vec3::Vec3;

impl Tiling {
    /// The axes of the plane the cells tile, as a right-handed pair with the
    /// extrusion axis: Z gives (X, Y), X gives (Y, Z), Y gives (Z, X). Keeping
    /// the cycle is what lets a cell built in the plane be carried into the
    /// world by a permutation of its coordinates, which cannot turn a solid
    /// inside out the way a reflection would.
    pub(super) fn plane_axes(&self) -> (usize, usize) {
        match self.axis {
            0 => (1, 2),
            1 => (2, 0),
            _ => (0, 1),
        }
    }

    /// A world point in the two directions of the plane the cells tile, which
    /// is what a picture of the tiling is drawn in.
    pub fn flatten(&self, p: Vec3) -> (f64, f64) {
        let (ua, va) = self.plane_axes();
        let p = [p.x, p.y, p.z];
        (p[ua], p[va])
    }

    /// A point of the cell frame -- across, along, through -- in world terms.
    ///
    /// The inverse of [`Tiling::flatten`] once a distance along the axis is
    /// named: `flatten` throws that distance away, because a plan of the cells
    /// does not need it, and lifting an outline back into the shape does.
    pub fn to_world(self, u: f64, v: f64, w: f64) -> Vec3 {
        let (ua, va) = self.plane_axes();
        let mut out = [0.0; 3];
        out[ua] = u;
        out[va] = v;
        out[self.axis.min(2) as usize] = w;
        Vec3::new(out[0], out[1], out[2])
    }

    /// The two side lengths of the grid's repeat, for the kinds laid out on a
    /// rectangular lattice.
    pub(super) fn steps(&self) -> (f64, f64) {
        let size = self.size.max(MIN_SIZE);
        match self.kind {
            CellKind::Squares => (size, size),
            CellKind::Rectangles => (size, self.depth.max(MIN_SIZE)),
            // A row of triangles repeats every side length across and every
            // triangle height along.
            CellKind::Triangles => (size, size * f64::sqrt(3.0) / 2.0),
            // Pointy-top hexagons: one width across, three quarters of the
            // point-to-point height along.
            CellKind::Hexagons => (size, size * f64::sqrt(3.0) / 2.0),
        }
    }

    /// How many cells a lattice row holds per step. Triangles are two per step
    /// -- one pointing each way -- and everything else is one.
    pub(super) fn per_step(&self) -> usize {
        if self.kind == CellKind::Triangles {
            2
        } else {
            1
        }
    }

    /// How many cells would be laid over a shape of these bounds, before any of
    /// them is found to be empty.
    ///
    /// Counted arithmetically rather than by building them, because the whole
    /// point is to be able to refuse a number too large to build.
    pub fn cell_count(&self, bounds: (Vec3, Vec3)) -> usize {
        let (across, along, through) = self.spans(bounds);
        let (sx, sy) = self.steps();
        let columns = (across / sx).ceil() as usize + 2;
        let rows = (along / sy).ceil() as usize + 2;
        columns.saturating_mul(rows).saturating_mul(self.per_step()).saturating_mul(self.layer_count(through))
    }

    /// How far the shape reaches across the grid's own two directions, and
    /// along the axis.
    ///
    /// The grid may be turned within its plane, so what has to be covered is
    /// the box of the *turned* corners, not the box itself.
    pub(super) fn spans(&self, bounds: (Vec3, Vec3)) -> (f64, f64, f64) {
        let (lo, hi) = bounds;
        let (ua, va) = self.plane_axes();
        let (lo, hi) = ([lo.x, lo.y, lo.z], [hi.x, hi.y, hi.z]);
        let (w, d) = (hi[ua] - lo[ua], hi[va] - lo[va]);
        let (sin, cos) = (self.angle.to_radians().sin().abs(), self.angle.to_radians().cos().abs());
        (w * cos + d * sin, w * sin + d * cos, hi[self.axis.min(2) as usize] - lo[self.axis.min(2) as usize])
    }

    /// How many layers the cells are cut into along the axis.
    pub(super) fn layer_count(&self, through: f64) -> usize {
        if self.layer < MIN_SIZE {
            1
        } else {
            ((through / self.layer).ceil() as usize).max(1)
        }
    }

    /// Whether this is a tiling that can be built at all: a size that is a size,
    /// and a cell count something can be done with.
    pub fn refusal(&self, bounds: (Vec3, Vec3)) -> Option<String> {
        if self.size < MIN_SIZE || (self.kind.has_depth() && self.depth < MIN_SIZE) {
            return Some("A cell needs a size bigger than zero.".to_string());
        }
        let cells = self.cell_count(bounds);
        if cells > MAX_CELLS {
            return Some(format!(
                "That is {cells} cells, and {MAX_CELLS} is as many as one split may make. \
                 Use a bigger cell, or split a part of the shape."
            ));
        }
        None
    }
}
