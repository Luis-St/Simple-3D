//! The shape one cell of a tiling is, and what each kind implies about how
//! the cells pack.

use serde::{Deserialize, Serialize};

/// The shape one cell of the tiling is.
///
/// Every one of them tiles the plane exactly -- no gaps and no overlaps -- which
/// is what makes the pieces add back up to the shape they were cut from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CellKind {
    #[default]
    Squares,
    Rectangles,
    Triangles,
    Hexagons,
}

impl CellKind {
    pub const ALL: [CellKind; 4] = [CellKind::Squares, CellKind::Rectangles, CellKind::Triangles, CellKind::Hexagons];

    pub fn label(self) -> &'static str {
        match self {
            CellKind::Squares => "Squares",
            CellKind::Rectangles => "Rectangles",
            CellKind::Triangles => "Triangles",
            CellKind::Hexagons => "Hexagons",
        }
    }

    /// The singular, for naming one piece.
    pub fn singular(self) -> &'static str {
        match self {
            CellKind::Squares => "square",
            CellKind::Rectangles => "rectangle",
            CellKind::Triangles => "triangle",
            CellKind::Hexagons => "hexagon",
        }
    }

    /// Whether the second side length means anything for this kind. Only a
    /// rectangle has two; the others are defined by one number, and offering a
    /// field that changes nothing is worse than not offering it.
    pub fn has_depth(self) -> bool {
        self == CellKind::Rectangles
    }

    /// What the one size number means for this kind, for a tooltip.
    pub fn size_meaning(self) -> &'static str {
        match self {
            CellKind::Squares => "The side of one square.",
            CellKind::Rectangles => "The first side of one rectangle.",
            CellKind::Triangles => {
                "The side of one triangle. They are equilateral, and alternate point-up and \
                                    point-down along a row."
            }
            CellKind::Hexagons => "The width of one hexagon across its flats.",
        }
    }
}
