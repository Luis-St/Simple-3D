//! What the modifier keys mean while a handle is dragged.

/// Which modifiers are down. Kept as a plain struct so the drag maths can be
/// tested without an egui context.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    /// Drag freely: no snapping at all.
    pub free: bool,
    /// Snap coarsely: ten times the increment.
    pub coarse: bool,
    /// Resize about the centre, or preserve proportions on a corner.
    pub symmetric: bool,
}

impl Mods {
    /// The increment to snap a value to, or `None` for a free drag.
    pub(super) fn increment(self, base: f64) -> Option<f64> {
        if self.free || base <= 0.0 {
            None
        } else if self.coarse {
            Some(base * 10.0)
        } else {
            Some(base)
        }
    }

    /// Round a dragged distance to the step in force -- also used by the
    /// pattern spacing handle (issue 67), so it snaps like every other drag.
    pub(crate) fn snap(self, value: f64, base: f64) -> f64 {
        match self.increment(base) {
            Some(step) => (value / step).round() * step,
            None => value,
        }
    }
}
