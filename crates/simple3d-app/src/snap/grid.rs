//! The ground grid as something to snap to (issue 126): its crossings and lines near the pointer,
//! and rounding a point onto the move grid.

use simple3d_geom::Vec3;

/// How many grid lines either side of the one under the pointer are offered. On a tilted ground the
/// nearest line on screen is not always the nearest on the ground.
const NEIGHBOURS: i64 = 3;

/// The drawn ground grid: its spacing, and the square it covers around its centre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundGrid {
    pub spacing: f64,
    pub centre: (f64, f64),
    pub half: f64,
}

impl GroundGrid {
    /// The crossings and line segments around `ground`, the ground point under the pointer. Only what
    /// lies on the drawn square, since a snap to a line that is not there would come out of nowhere.
    pub fn near(&self, ground: Vec3) -> (Vec<Vec3>, Vec<(Vec3, Vec3)>) {
        let s = self.spacing;
        if s <= 0.0 {
            return (Vec::new(), Vec::new());
        }
        let (cx, cy) = self.centre;
        let inside = |v: f64, c: f64| (v - c).abs() <= self.half + 1e-9;
        let (ix, iy) = ((ground.x / s).round() as i64, (ground.y / s).round() as i64);
        let xs: Vec<f64> = (-NEIGHBOURS..=NEIGHBOURS).map(|i| (ix + i) as f64 * s).filter(|&x| inside(x, cx)).collect();
        let ys: Vec<f64> = (-NEIGHBOURS..=NEIGHBOURS).map(|i| (iy + i) as f64 * s).filter(|&y| inside(y, cy)).collect();
        let points = xs.iter().flat_map(|&x| ys.iter().map(move |&y| Vec3::new(x, y, 0.0))).collect();
        let mut lines = Vec::new();
        for &x in &xs {
            lines.push((Vec3::new(x, cy - self.half, 0.0), Vec3::new(x, cy + self.half, 0.0)));
        }
        for &y in &ys {
            lines.push((Vec3::new(cx - self.half, y, 0.0), Vec3::new(cx + self.half, y, 0.0)));
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
