//! What of a step falls in this band, and inside the frame at all.

use super::*;
impl Frame<'_> {
    /// The run of step indices, out of `0..=steps`, whose sample rows can fall
    /// inside this band -- widened by one at each end so rounding can never
    /// drop a sample the band should have drawn.
    pub(super) fn steps_in_band(&self, a: Vertex, b: Vertex, steps: usize) -> (usize, usize) {
        let dy = b.pos.y - a.pos.y;
        if dy == 0.0 {
            let row = a.pos.y;
            if row < self.row_lo as f32 || row >= self.row_hi as f32 {
                return (1, 0); // an empty run
            }
            return (0, steps);
        }
        let at = |y: f32| (y - a.pos.y) / dy * steps as f32;
        let (lo, hi) = (at(self.row_lo as f32), at(self.row_hi as f32));
        let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
        let first = (lo.floor().max(0.0) as usize).saturating_sub(1);
        let last = ((hi.ceil().max(0.0) as usize) + 1).min(steps);
        (first, last)
    }

    /// Liang-Barsky clip of a segment against the framebuffer rectangle,
    /// interpolating the depth key along with the position.
    pub(super) fn clip_to_frame(&self, a: Vertex, b: Vertex) -> Option<(Vertex, Vertex)> {
        let (mut t0, mut t1) = (0.0f32, 1.0f32);
        let dx = b.pos.x - a.pos.x;
        let dy = b.pos.y - a.pos.y;
        // The framebuffer's last addressable pixel centre, with a little slack so
        // a line exactly on the edge still draws.
        let limits = [
            (-dx, a.pos.x - 0.0),
            (dx, (self.width as f32 - 0.001) - a.pos.x),
            (-dy, a.pos.y - 0.0),
            (dy, (self.height as f32 - 0.001) - a.pos.y),
        ];
        for (p, q) in limits {
            if p == 0.0 {
                if q < 0.0 {
                    return None; // parallel to this edge and outside it
                }
            } else {
                let r = q / p;
                if p < 0.0 {
                    if r > t1 {
                        return None;
                    }
                    t0 = t0.max(r);
                } else {
                    if r < t0 {
                        return None;
                    }
                    t1 = t1.min(r);
                }
            }
        }
        if t0 > t1 {
            return None;
        }
        let at =
            |t: f32| Vertex { pos: egui::pos2(a.pos.x + dx * t, a.pos.y + dy * t), key: a.key + (b.key - a.key) * t };
        Some((at(t0), at(t1)))
    }

    #[cfg(test)]
    pub(super) fn pixel(&self, x: usize, y: usize) -> Rgba {
        let o = (y * self.width + x) * 4;
        [self.color[o], self.color[o + 1], self.color[o + 2], self.color[o + 3]]
    }
}
