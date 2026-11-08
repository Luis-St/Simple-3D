//! A renderable edited in place: the brush's working copy (`crate::sculpt`, issue 127).
//!
//! Its positions, triangle normals and corner normals are kept current by the brush; its feature
//! edges, outline, bodies and axis spans stay as they were when it was made, since the triangles do
//! not change and a stroke's few millimetres do not move a crease or a body. The evaluation that
//! follows the stroke makes everything afresh.

use super::*;

impl Renderable {
    /// A new [`Renderable::id`], so the GPU uploads the copy whole: at the start of each stroke,
    /// rather than trusting a copy on the card the previous one left.
    pub(crate) fn renew_id(&mut self) {
        self.id = next_id();
    }

    /// Forget the plane marks after vertices moved, so they are found again where they are now.
    pub(crate) fn forget_plane_marks(&mut self) {
        self.plane_marks = OnceLock::new();
    }
}
