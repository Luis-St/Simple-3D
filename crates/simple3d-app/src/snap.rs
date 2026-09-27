//! Snap features: notable points on a body that drags catch and the measure tool picks (issues 68,
//! 69): vertices, edge midpoints and flat face centres, in world space.
//!
//! Meshes arrive triangulated and unwelded, so they are welded and coplanar triangles merged back
//! into faces, giving one vertex per corner and one centre per face.

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

/// How near the pointer a feature must project to be caught, in pixels; shared by measuring and snapping.
pub const CATCH_PIXELS: f32 = 12.0;

/// How near a dragged body's feature must come to another's to catch, in pixels. Wider than
/// [`CATCH_PIXELS`], since it is a whole body steered from a distant handle, not a pointer aimed finely.
pub const DRAG_CATCH_PIXELS: f32 = 22.0;
