//! Shared surface-of-revolution / extrusion helpers used by the primitive
//! generators. All of these build a "grid" of rings and triangulate the grid
//! with a winding convention validated to face outward:
//! for adjacent rings/profile-samples `V(j,i)`, `V(j2,i)`, `V(j2,i+1)`, `V(j,i+1)`
//! the two triangles `(V(j,i), V(j2,i), V(j2,i+1))` and
//! `(V(j,i), V(j2,i+1), V(j,i+1))` face outward when `j` sweeps CCW (viewed
//! from +Z) and the cross-section is traversed bottom-to-top / CCW as seen
//! in the (radius, z) half-plane.

mod outline;
pub use outline::{chamfer_rect_outline, inset_convex_outline, ring_outline, rounded_rect_outline, sector_outline};
mod extrude;
pub use extrude::{extrude_frustum_polygon, extrude_stack};
mod profile;
pub use profile::{revolve_closed_profile, revolve_open_profile};
