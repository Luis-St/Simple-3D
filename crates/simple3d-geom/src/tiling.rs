//! Cutting one solid into a tiling of smaller solids (issue 82).
//!
//! The shape is in one piece, and the pieces are made by cutting it -- into
//! squares, rectangles, triangles or hexagons, the way a plate is scored before
//! it is broken.
//!
//! The cells are prisms: a 2D tiling of the plane, extruded along one axis
//! through the whole solid, and optionally cut into layers along that axis as
//! well. Each piece is the intersection of the solid with one cell, so the
//! pieces are disjoint, they add back up to the solid, and each one keeps the
//! part of the original surface it had. Nothing here knows about scenes or
//! nodes: it takes a mesh and gives back meshes.
//!
//! Two things keep it affordable. A cell no triangle of the solid comes near is
//! either wholly inside it or wholly outside it, which one ray answers -- so the
//! inside of a big shape costs no boolean at all and only the cells along the
//! surface go through the kernel. And the cells are independent, so they are cut
//! on as many threads as the machine has.

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

/// How a solid is to be cut into pieces (issue 82).
///
/// Sizes are millimetres and the angle is degrees, as everywhere else in the
/// document. `axis` is the direction the cells are extruded along -- the tiling
/// itself lies in the plane across it -- so a plate lying flat is cut into
/// columns by the default, `Z`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tiling {
    pub kind: CellKind,
    /// The cell's size across, in millimetres. What it measures depends on the
    /// kind -- see [`CellKind::size_meaning`].
    pub size: f64,
    /// The second side of a rectangle, in millimetres. Ignored by every other
    /// kind, but kept through a change of kind so switching back and forth does
    /// not lose the number.
    pub depth: f64,
    /// The axis the cells run along: 0 = X, 1 = Y, 2 = Z.
    pub axis: u8,
    /// Degrees the whole grid is turned by within its plane.
    pub angle: f64,
    /// Where the grid sits, relative to the centre of what is being cut, in the
    /// two axes of its plane. Zero puts a cell's centre on the shape's centre,
    /// which is the symmetric answer; shifting it is how a cut is moved off a
    /// feature it would otherwise land on.
    pub offset: [f64; 2],
    /// Also cut into layers of this height along the axis. Zero -- the default
    /// -- cuts straight through, which is what "split into hexagons" means on
    /// its own.
    pub layer: f64,
}

impl Default for Tiling {
    fn default() -> Tiling {
        Tiling { kind: CellKind::Squares, size: 10.0, depth: 10.0, axis: 2, angle: 0.0, offset: [0.0, 0.0], layer: 0.0 }
    }
}

/// The smallest cell worth asking for. Below this the number of cells runs away
/// faster than the arithmetic that counts them is worth doing.
pub const MIN_SIZE: f64 = 0.01;

/// The most cells a single split may ask for.
///
/// Not a performance guess but a usability one: ten thousand pieces is ten
/// thousand rows in the outliner and ten thousand bodies in an export, and a
/// number typed with one zero too many should be refused while it can still be
/// corrected rather than after a minute of cutting.
pub const MAX_CELLS: usize = 10_000;
