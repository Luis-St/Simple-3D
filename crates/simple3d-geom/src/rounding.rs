//! Rounding and bevelling the edges and corners of a solid (issue 88), by booleans.
//!
//! An edge is rounded by cutting away the material between its two faces and the arc tangent to
//! both: a prism along the edge whose cross-section is that sliver (a convex edge), or by adding
//! the same sliver on the air side (a concave edge, a fillet). A bevel cuts or adds the triangle
//! under a straight chord instead. Corners where three faces meet are blended by a block minus a
//! ball, or cut off by a plane.
//!
//! The solids are made from an edge or corner found on a model and handed back to the caller, which
//! keeps the edges and corners as an edit of the object they belong to and remakes the solids when
//! it is evaluated, so the treatment's size can still be changed and the rounding taken back.
//!
//! Every cutter overshoots into the air around the edge, so no face of it lies on a face of the
//! solid; edge cutters also run on past an end that opens into air, so the cutters of a curved run
//! of short edges overlap rather than leave slivers between them.

mod edges;
pub use edges::{feature_edges, feature_edges_by, FeatureEdge, FEATURE_DEG};
mod profile;
pub use profile::{edge_profile, edge_solid, edge_solid_ends, run_on, Profile};
mod mitre;
pub use mitre::{edge_ends, inside_corners, End};
mod corner;
pub use corner::{corner_chamfer, corner_round, Corner};
mod sliver;
pub use sliver::{corner_sliver, edge_sliver};
mod room;
pub use room::{corner_fits, edge_fits, edge_room};
mod extend;
pub use extend::extend_edge;
mod inside;
pub use inside::contains_point;
#[cfg(test)]
mod tests;

/// What is done to an edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Treatment {
    /// A circular arc of `radius`, in `segments` straight pieces.
    Round { radius: f64, segments: u32 },
    /// A straight cut `distance` back along each face.
    Chamfer { distance: f64 },
}

impl Treatment {
    /// How far back along each face the treatment reaches, for a wedge of `angle` radians between them.
    pub fn reach(self, angle: f64) -> f64 {
        match self {
            Treatment::Round { radius, .. } => radius / (angle / 2.0).tan(),
            Treatment::Chamfer { distance } => distance,
        }
    }
}
