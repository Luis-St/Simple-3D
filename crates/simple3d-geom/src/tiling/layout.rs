//! A tiling's own numbers: its axes, its step, and its cell and layer counts.

use super::*;
use crate::vec3::Vec3;

impl Tiling {
    /// The cell plane's axes as a right-handed pair with the extrusion axis (Z: X, Y; X: Y, Z; Y: Z, X),
    /// so cells map to the world by a permutation that cannot mirror a solid.
    pub(super) fn plane_axes(&self) -> (usize, usize) {
        match self.axis {
            0 => (1, 2),
            1 => (2, 0),
            _ => (0, 1),
        }
    }

    /// A world point in the cell plane's two directions, for drawing the tiling.
    pub fn flatten(&self, p: Vec3) -> (f64, f64) {
        let (ua, va) = self.plane_axes();
        let p = [p.x, p.y, p.z];
        (p[ua], p[va])
    }

    /// A cell-frame point (across, along, through) in world terms; the inverse of [`Tiling::flatten`]
    /// given the distance along the axis.
    pub fn to_world(self, u: f64, v: f64, w: f64) -> Vec3 {
        let (ua, va) = self.plane_axes();
        let mut out = [0.0; 3];
        out[ua] = u;
        out[va] = v;
        out[self.axis.min(2) as usize] = w;
        Vec3::new(out[0], out[1], out[2])
    }

    /// The grid repeat's two side lengths, for rectangular lattices.
    pub(super) fn steps(&self) -> (f64, f64) {
        let size = self.size.max(MIN_SIZE);
        match self.kind {
            CellKind::Squares => (size, size),
            CellKind::Rectangles => (size, self.depth.max(MIN_SIZE)),
            // Triangles repeat every side across and every triangle height along.
            CellKind::Triangles => (size, size * f64::sqrt(3.0) / 2.0),
            // Pointy-top hexagons: one width across, three quarters of the point-to-point height along.
            CellKind::Hexagons => (size, size * f64::sqrt(3.0) / 2.0),
        }
    }

    /// Cells per lattice step: two for triangles (one each way), else one.
    pub(super) fn per_step(&self) -> usize {
        if self.kind == CellKind::Triangles {
            2
        } else {
            1
        }
    }

    /// How many cells would cover a shape of these bounds, counted arithmetically so an absurd number
    /// can be refused before building.
    pub fn cell_count(&self, bounds: (Vec3, Vec3)) -> usize {
        let (across, along, through) = self.spans(bounds);
        let (sx, sy) = self.steps();
        let columns = (across / sx).ceil() as usize + 2;
        let rows = (along / sy).ceil() as usize + 2;
        columns.saturating_mul(rows).saturating_mul(self.per_step()).saturating_mul(self.layer_count(through))
    }

    /// The shape's reach across the grid's two directions and along the axis, using the turned
    /// corners' box.
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

    /// Why this tiling cannot be built, if so: a non-size, or too many cells.
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
