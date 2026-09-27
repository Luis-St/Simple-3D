//! What the modifier keys mean while a handle is dragged.

/// Modifiers held, as a plain struct so drag maths is testable without egui.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    /// No snapping.
    pub free: bool,
    /// Snap at ten times the increment.
    pub coarse: bool,
    /// Resize about the centre, or keep proportions on a corner.
    pub symmetric: bool,
}

impl Mods {
    /// The snap increment, or `None` for a free drag.
    pub(super) fn increment(self, base: f64) -> Option<f64> {
        if self.free || base <= 0.0 {
            None
        } else if self.coarse {
            Some(base * 10.0)
        } else {
            Some(base)
        }
    }

    /// Round a dragged distance to the current step; the pattern spacing handle uses it too (issue 67).
    pub(crate) fn snap(self, value: f64, base: f64) -> f64 {
        match self.increment(base) {
            Some(step) => (value / step).round() * step,
            None => value,
        }
    }
}
