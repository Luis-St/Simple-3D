//! Surface-of-revolution and extrusion helpers for the primitive generators. They triangulate ring grids
//! so that for adjacent samples `V(j,i)`, `V(j2,i)`, `V(j2,i+1)`, `V(j,i+1)` the triangles
//! `(V(j,i), V(j2,i), V(j2,i+1))` and `(V(j,i), V(j2,i+1), V(j,i+1))` face outward, given `j` sweeps CCW
//! from +Z and the cross-section runs bottom-to-top / CCW in the (radius, z) half-plane.

mod outline;
pub use outline::{chamfer_rect_outline, inset_convex_outline, ring_outline, rounded_rect_outline, sector_outline};
mod extrude;
pub use extrude::{extrude_frustum_polygon, extrude_stack};
mod profile;
pub use profile::{revolve_closed_profile, revolve_open_profile};
