//! The frame's depth range.

/// Min and max depth key in the frame, widened by everything drawn, mapped onto the depth buffer.
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
