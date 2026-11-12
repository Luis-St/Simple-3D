//! Functions run over a mesh's surface (issue 81): noise, ridges, cells, smoothing and inflating.
//!
//! Every function moves vertices; none adds or removes a triangle. A coarse mesh is first refined
//! ([`refine`]) so the surface has the vertices to show detail the size of [`Shaping::scale`].
//! The moving functions push each vertex along its normal ([`crate::topology::vertex_normals`]) or
//! along an axis, by the function's value times the vertex's weight ([`weights`]), which is how a hole's
//! rim and a print's base are kept in place. Inflating moves each vertex by its offset vector instead
//! ([`offset::offset_directions`]), so a box's faces move out by the amount right to their edges.

mod noise;
pub use noise::Noise;
mod refine;
pub use refine::refine_until;
pub(crate) use refine::split_longest;
mod offset;
mod smooth;
mod weights;
pub use weights::weights;
mod run;
pub use run::{shape, shape_until};
#[cfg(test)]
mod tests;

use crate::mesh::Mesh;
use serde::{Deserialize, Serialize};

/// What to run over the surface, and how strongly. Read per [`Function`]: the noise-like ones use
/// every field, Smooth reads `layers` as passes and ignores `amount`, `scale` and `seed`, Inflate
/// reads `amount` only.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Shaping {
    pub function: Function,
    /// How far the surface moves at most, in millimetres; negative pushes inwards.
    pub amount: f64,
    /// The size of the pattern's features, in millimetres: the noise's wavelength or a cell's width.
    pub scale: f64,
    /// Layers of ever finer detail on top of the first, each half the size and half the strength;
    /// for Smooth, how many passes are run.
    pub layers: u32,
    /// Which of the endless patterns of a kind this is.
    pub seed: u32,
    pub direction: Direction,
    /// Whether the mesh is first split until no edge is longer than `edge`.
    pub refine: bool,
    /// The longest edge refining leaves, in millimetres. Kept while `refine` is off.
    pub edge: f64,
    /// Whether a hole's rim stays where it is, so an open surface keeps its outline.
    pub keep_boundaries: bool,
    /// Whether the bottom of the mesh stays where it is, so a print still sits flat.
    pub keep_base: bool,
    /// How high above the lowest point the surface is kept, in millimetres, in the mesh's own frame;
    /// the function fades in over the same height again above it. Kept while `keep_base` is off.
    pub base: f64,
}

impl Default for Shaping {
    fn default() -> Shaping {
        Shaping {
            function: Function::Noise,
            amount: 1.0,
            scale: 10.0,
            layers: 3,
            seed: 1,
            direction: Direction::Normal,
            refine: true,
            edge: 1.0,
            keep_boundaries: true,
            keep_base: false,
            base: 1.0,
        }
    }
}

/// The functions there are.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Function {
    /// Smooth random bumps both ways (Perlin noise).
    #[default]
    Noise,
    /// Sharp crests rising out of the surface, as on rock.
    Ridges,
    /// Rounded cells pushed out, with creases between them, as on hammered metal or scales.
    Cells,
    /// Pull each vertex towards its neighbours, taking out bumps and keeping the volume.
    Smooth,
    /// Push the whole surface out along its normals by `amount`.
    Inflate,
}

impl Function {
    pub const ALL: [Function; 5] =
        [Function::Noise, Function::Ridges, Function::Cells, Function::Smooth, Function::Inflate];

    pub fn label(self) -> &'static str {
        match self {
            Function::Noise => "Noise",
            Function::Ridges => "Ridges",
            Function::Cells => "Cells",
            Function::Smooth => "Smooth",
            Function::Inflate => "Inflate",
        }
    }

    /// Whether it draws a pattern, and so reads `scale`, `layers` and `seed`.
    pub fn patterned(self) -> bool {
        matches!(self, Function::Noise | Function::Ridges | Function::Cells)
    }
}

/// Which way a vertex moves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Out of the surface, so the shape keeps its outline and gains texture.
    #[default]
    Normal,
    X,
    Y,
    Z,
}

impl Direction {
    pub const ALL: [Direction; 4] = [Direction::Normal, Direction::X, Direction::Y, Direction::Z];

    pub fn label(self) -> &'static str {
        match self {
            Direction::Normal => "Along the surface normal",
            Direction::X => "Along X",
            Direction::Y => "Along Y",
            Direction::Z => "Along Z",
        }
    }
}

/// The most triangles refining makes; it stops short of a finer edge rather than pass this.
pub const MAX_TRIANGLES: usize = 2_000_000;

/// What a run produced.
pub struct Outcome {
    pub mesh: Mesh,
    /// Whether refining stopped at [`MAX_TRIANGLES`] before every edge was short enough.
    pub capped: bool,
    /// The furthest any vertex moved, in millimetres.
    pub moved: f64,
}
