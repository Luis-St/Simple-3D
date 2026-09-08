//! Clicking geometry in the viewport selects the corresponding node in the
//! outliner (spec section 6.1, acceptance criterion 12).
//!
//! The ray is cast against each node's *own* world-space mesh rather than
//! against the evaluated result, because the evaluated result is one merged mesh
//! with no memory of where its triangles came from. A consequence worth knowing:
//! clicking the wall of a drilled hole selects the cylinder that cut it, which is
//! the node you would want to adjust.

mod ray;
pub use ray::ray_mesh;
mod pick;
pub use pick::pick;
#[cfg(test)]
mod tests;
