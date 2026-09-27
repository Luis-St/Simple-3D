//! The shape of one tiling cell.

use serde::{Deserialize, Serialize};

/// The shape of one cell. Each tiles the plane exactly, so the pieces add back up to the original.
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

    /// Whether the second side length applies; only rectangles have two.
    pub fn has_depth(self) -> bool {
        self == CellKind::Rectangles
    }

    /// What the size number means for this kind, for a tooltip.
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
