//! Snap features: the notable points on a body's surface that a drag can catch
//! onto and the measure tool can pick (issues 68, 69).
//!
//! A body offers three kinds of them -- its vertices, the midpoints of its edges
//! and the centres of its flat faces -- and both features that pick geometry
//! reach for the same three, so they live in one place. The input is a body's
//! own world-space mesh, the one `Evaluated::node_meshes` already holds, so a
//! feature is in world space from the moment it is found and nothing has to
//! transform it again.
//!
//! Meshes arrive triangulated, with a box face split into two triangles and each
//! triangle carrying its own unwelded copies of its corners. Left as they are,
//! "vertices" would be three per triangle and "face centres" would be triangle
//! centroids -- two of them per box face, neither at the face's middle. So the
//! mesh is welded first, which recovers the shared vertices, and coplanar
//! triangles are merged back into the flat face they came from, which recovers
//! the one centre a user means by it.

mod feature;
pub use feature::{Feature, FeatureKind};
mod body;
pub use body::features_of;
mod axis;
pub use axis::{axis_features, axis_lines};
mod plane;
pub(crate) use plane::*;
pub use plane::{plane_crossing, plane_mark_lines, MARK_AXIS};
mod nearest;
pub use nearest::{near_on_screen, nearest_on_edge};
#[cfg(test)]
mod tests;

/// How near the pointer a feature has to project to be caught, in screen pixels.
/// One radius for the measure tool and for geometry snapping, so a feature feels
/// the same to reach for whichever is doing the reaching.
pub const CATCH_PIXELS: f32 = 12.0;

/// How near a feature of the body being dragged has to come to a feature of
/// another body for the drag to catch on it, in screen pixels.
///
/// Wider than [`CATCH_PIXELS`], and deliberately: that is a pointer reaching for
/// one exact point, which is aimed and can be aimed finely. This is a whole body
/// coming alongside another, judged by eye at whatever zoom the viewport happens
/// to be at and steered by a handle the pointer is nowhere near -- twelve pixels
/// of that is a gap a drag cannot be aimed into, and a snap nobody ever feels.
pub const DRAG_CATCH_PIXELS: f32 = 22.0;
