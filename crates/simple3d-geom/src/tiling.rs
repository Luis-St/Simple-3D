//! Cutting one solid into a tiling of smaller solids (issue 82).
//!
//! Cells are prisms (a 2D tiling of squares, rectangles, triangles or hexagons extruded along an
//! axis, optionally layered). Each piece is the solid intersected with a cell, so pieces are
//! disjoint and sum to the solid. Cells no triangle comes near are wholly in or out, settled by one
//! ray, so only surface cells use the kernel; cells are cut in parallel.

mod cell_kind;
pub use cell_kind::CellKind;
mod layout;
mod split_plan;
pub use split_plan::{SplitPlan, MAX_PASSES};
mod plan;
pub use plan::cut_plan;
pub(crate) use plan::*;
mod arrange;
pub use arrange::planned;
pub(crate) use arrange::*;
mod shapes;
pub(crate) use shapes::*;
mod outline;
pub use outline::{cell_outlines, preview_loops};
mod cut;
pub use cut::cut;
pub(crate) use cut::*;
#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

/// How a solid is to be cut (issue 82). Millimetres and degrees; `axis` is the extrusion direction,
/// so a flat plate is cut into columns by the default Z.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tiling {
    pub kind: CellKind,
    /// The cell's size across, in millimetres (see [`CellKind::size_meaning`]).
    pub size: f64,
    /// A rectangle's second side, in millimetres; kept across kind changes.
    pub depth: f64,
    /// The axis the cells run along: 0 = X, 1 = Y, 2 = Z.
    pub axis: u8,
    /// Degrees the grid is turned within its plane.
    pub angle: f64,
    /// The grid's offset from the shape's centre in its plane; zero centres a cell on the shape.
    pub offset: [f64; 2],
    /// Layer height along the axis; zero cuts straight through.
    pub layer: f64,
}

impl Default for Tiling {
    fn default() -> Tiling {
        Tiling { kind: CellKind::Squares, size: 10.0, depth: 10.0, axis: 2, angle: 0.0, offset: [0.0, 0.0], layer: 0.0 }
    }
}

/// The smallest cell worth asking for.
pub const MIN_SIZE: f64 = 0.01;

/// The most cells one split may ask for: a usability limit, refusing typos before minutes of cutting.
pub const MAX_CELLS: usize = 10_000;
