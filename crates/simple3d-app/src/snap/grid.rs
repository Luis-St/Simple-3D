//! The grid as something to snap to (issue 126): its crossings and lines near the pointer, on the
//! ground or the active workplane (issue 125), and rounding a point onto the move grid.

use crate::render::GridPlane;
use simple3d_geom::Vec3;

/// How many grid lines either side of the one under the pointer are offered. On a tilted grid the
/// nearest line on screen is not always the nearest on the plane.
const NEIGHBOURS: i64 = 3;

/// The drawn grid: its plane, its spacing, and the square it covers around its centre, in the
/// plane's own coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundGrid {
    pub plane: GridPlane,
    pub spacing: f64,
    pub centre: (f64, f64),
    pub half: f64,
}

impl GroundGrid {
    /// The crossings and line segments around `ground`, the point on the grid's plane under the
    /// pointer. Only what lies on the drawn square, since a snap to a line that is not there would come
    /// out of nowhere.
    pub fn near(&self, ground: Vec3) -> (Vec<Vec3>, Vec<(Vec3, Vec3)>) {
        let s = self.spacing;
        if s <= 0.0 {
            return (Vec::new(), Vec::new());
        }
        let (cx, cy) = self.centre;
        let (gx, gy) = self.plane.local(ground);
        let inside = |v: f64, c: f64| (v - c).abs() <= self.half + 1e-9;
        let (ix, iy) = ((gx / s).round() as i64, (gy / s).round() as i64);
        let xs: Vec<f64> = (-NEIGHBOURS..=NEIGHBOURS).map(|i| (ix + i) as f64 * s).filter(|&x| inside(x, cx)).collect();
        let ys: Vec<f64> = (-NEIGHBOURS..=NEIGHBOURS).map(|i| (iy + i) as f64 * s).filter(|&y| inside(y, cy)).collect();
        let at = |x: f64, y: f64| self.plane.at(x, y);
        let points = xs.iter().flat_map(|&x| ys.iter().map(move |&y| at(x, y))).collect();
        let mut lines = Vec::new();
        for &x in &xs {
            lines.push((at(x, cy - self.half), at(x, cy + self.half)));
        }
        for &y in &ys {
            lines.push((at(cx - self.half, y), at(cx + self.half, y)));
        }
        (points, lines)
    }
}

/// `at` rounded onto the move grid of `step`. On a surface square to an axis only along the two axes
/// it lies in, so the point stays on it; a slanted surface has no grid to keep to. Without a normal,
/// along all three.
pub fn on_grid(at: Vec3, normal: Option<Vec3>, step: f64) -> Vec3 {
    if step <= 0.0 {
        return at;
    }
    let across = match normal {
        Some(normal) => match (0..3).find(|&axis| normal.get(axis).abs() > 1.0 - 1e-9) {
            Some(axis) => Some(axis),
            None => return at,
        },
        None => None,
    };
    let mut out = at;
    for axis in (0..3).filter(|&axis| Some(axis) != across) {
        out.set(axis, (at.get(axis) / step).round() * step);
    }
    out
}
