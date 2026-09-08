//! The outline of a single cell, per tiling kind.

use super::*;
use std::f64::consts::PI;

/// One cell as it is laid out: where its centre is, and its outline
/// counter-clockwise about it.
pub(crate) type CellShape = ((f64, f64), Vec<(f64, f64)>);

/// The cell or cells at one place in the lattice, in the grid's own frame.
pub(crate) fn cell_shapes(tiling: &Tiling, i: i64, j: i64) -> Vec<CellShape> {
    let (sx, sy) = tiling.steps();
    let (i, j) = (i as f64, j as f64);
    match tiling.kind {
        CellKind::Squares | CellKind::Rectangles => {
            let centre = (i * sx, j * sy);
            let (hw, hd) = (sx / 2.0, sy / 2.0);
            let outline = vec![
                (centre.0 - hw, centre.1 - hd),
                (centre.0 + hw, centre.1 - hd),
                (centre.0 + hw, centre.1 + hd),
                (centre.0 - hw, centre.1 + hd),
            ];
            vec![(centre, outline)]
        }
        // Two per step: one pointing up whose base is the bottom of the row,
        // and one pointing down between it and the next, whose base is the top.
        // Together they fill the row exactly.
        CellKind::Triangles => {
            let radius = sx / f64::sqrt(3.0);
            let up = (i * sx + sx / 2.0, j * sy + sy / 3.0);
            let down = (i * sx + sx, j * sy + 2.0 * sy / 3.0);
            vec![(up, triangle(up, radius, false)), (down, triangle(down, radius, true))]
        }
        // Rows interlock, so every other one is shifted half a cell across.
        CellKind::Hexagons => {
            let shift = if j.rem_euclid(2.0) == 0.0 { 0.0 } else { sx / 2.0 };
            let centre = (i * sx + shift, j * sy);
            vec![(centre, hexagon(centre, sx / f64::sqrt(3.0)))]
        }
    }
}

/// An equilateral triangle about its own centre, point up or point down.
pub(crate) fn triangle(centre: (f64, f64), radius: f64, down: bool) -> Vec<(f64, f64)> {
    let turn = if down { PI } else { 0.0 };
    (0..3)
        .map(|k| {
            let angle = turn + PI / 2.0 + k as f64 * 2.0 * PI / 3.0;
            (centre.0 + radius * angle.cos(), centre.1 + radius * angle.sin())
        })
        .collect()
}

/// A pointy-top hexagon about its own centre. `radius` is corner to centre,
/// which is the width across the flats over the square root of three.
pub(crate) fn hexagon(centre: (f64, f64), radius: f64) -> Vec<(f64, f64)> {
    (0..6)
        .map(|k| {
            let angle = PI / 2.0 + k as f64 * PI / 3.0;
            (centre.0 + radius * angle.cos(), centre.1 + radius * angle.sin())
        })
        .collect()
}
