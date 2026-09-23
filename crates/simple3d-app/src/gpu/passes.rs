//! The depth range one frame is drawn into, and the axis bias.

/// The smallest and largest depth key in the frame, so the whole scene can be
/// mapped into the depth buffer's range. Everything drawn widens it -- the
/// meshes by their boxes, the grid by its quad, the axes by their ends.
#[derive(Default)]
pub(crate) struct Passes {
    pub(super) key_range: Option<(f32, f32)>,
}

impl Passes {
    pub(super) fn saw(&mut self, key: f32) {
        self.key_range = Some(match self.key_range {
            None => (key, key),
            Some((lo, hi)) => (lo.min(key), hi.max(key)),
        });
    }
}

/// The axis bias, matching `render.rs`'s own.
pub(crate) const AXIS_BIAS: f32 = -5.0e-4;
