//! Drawing a line.

use super::*;
impl Frame<'_> {
    /// Draw a depth-tested line; `bias` nudges it towards the eye so an edge is not swallowed by its
    /// own face. Clipped to the framebuffer before stepping, since grid and axis lines run far outside.
    pub fn line(&mut self, a: Vertex, b: Vertex, rgba: Rgba, bias: f32) {
        self.line_with_depth(a, b, rgba, bias, true);
    }

    /// Draw a line; `write_depth` false leaves the depth buffer alone, for decoration that must
    /// never win a depth tie against the model.
    pub fn line_with_depth(&mut self, a: Vertex, b: Vertex, rgba: Rgba, bias: f32, write_depth: bool) {
        self.line_inner(a, b, rgba, bias, write_depth, None);
    }

    /// A line drawn over everything with depth ignored both ways, like `triangle_over`: an infinite
    /// bias wins every depth test.
    pub fn line_over(&mut self, a: Vertex, b: Vertex, rgba: Rgba) {
        self.line_inner(a, b, rgba, f32::INFINITY, false, None);
    }

    /// A line not hidden by the items marked in `through` (indexed by tag); others hide it as usual,
    /// and it writes no depth.
    pub fn line_through(&mut self, a: Vertex, b: Vertex, rgba: Rgba, bias: f32, through: &[bool]) {
        self.line_inner(a, b, rgba, bias, false, Some(through));
    }

    pub(super) fn line_inner(
        &mut self,
        a: Vertex,
        b: Vertex,
        rgba: Rgba,
        bias: f32,
        write_depth: bool,
        through: Option<&[bool]>,
    ) {
        // Clipped to the whole frame, never the band, so sample spacing matches the unbanded line.
        let Some((a, b)) = self.clip_to_frame(a, b) else { return };
        let steps = ((b.pos.x - a.pos.x).abs().max((b.pos.y - a.pos.y).abs()).ceil() as usize).max(1);
        // The samples landing in this band form one run, since `y` is monotonic; the rest are skipped.
        let (first, last) = self.steps_in_band(a, b, steps);
        for step in first..=last {
            let t = step as f32 / steps as f32;
            let x = a.pos.x + (b.pos.x - a.pos.x) * t;
            let y = a.pos.y + (b.pos.y - a.pos.y) * t;
            if x < 0.0 || y < 0.0 {
                continue;
            }
            let (x, y) = (x as usize, y as usize);
            if x >= self.width || y >= self.height {
                continue;
            }
            let key = a.key + (b.key - a.key) * t;
            self.put_with(x, y, key + bias, rgba, write_depth, through);
        }
    }
}
