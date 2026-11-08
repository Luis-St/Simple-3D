//! Clicking geometry selects its node (spec section 6.1, criterion 12). Rays test each node's own mesh,
//! since the merged result forgets its sources; so a drilled hole's wall selects its cutter.

mod bvh;
#[cfg(test)]
pub(crate) use bvh::MIN_TRIANGLES;
pub(crate) use bvh::{Bvh, Refit};
mod ray;
pub use ray::{ray_mesh, ray_mesh_triangle};
mod pick;
pub use pick::pick;
mod surface;
pub(crate) use surface::surface_hit;
#[cfg(test)]
mod tests;
