//! Drawing a line.

use super::*;
impl Frame<'_> {
    /// Draw a line, depth-tested. `bias` nudges it towards the eye so an edge
    /// drawn on the face it belongs to is not swallowed by it.
    ///
    /// The segment is clipped to the framebuffer *before* it is stepped along,
    /// rather than tested per pixel. That matters for the origin axes and the
    /// ground grid, whose lines run far outside the viewport: stepping them
    /// end to end would cost thousands of rejected samples each.
    /// Draw a line. `write_depth` false leaves the depth buffer alone, for
    /// decoration -- a grid, an axis -- that must never win a depth tie against
    /// the model it is drawn under.
    pub fn line(&mut self, a: Vertex, b: Vertex, rgba: Rgba, bias: f32) {
        self.line_with_depth(a, b, rgba, bias, true);
    }

    pub fn line_with_depth(&mut self, a: Vertex, b: Vertex, rgba: Rgba, bias: f32, write_depth: bool) {
        self.line_inner(a, b, rgba, bias, write_depth, None);
    }

    /// A line the items in `through` do not hide -- they are indexed by tag, so
    /// `through[tag]` says whether the item drawn under that tag is one the line
    /// is seen through. Everything else hides it as usual, and the line leaves
    /// no depth of its own.
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
        // Clipped to the whole frame, never to this band: the step count and so
        // the position of every sample along the line come out of these two
        // endpoints, and clipping them to the band would re-space the samples.
        // A banded frame has to draw the same line the whole frame would, and
        // then keep the part of it that is its own.
        let Some((a, b)) = self.clip_to_frame(a, b) else { return };
        let steps = ((b.pos.x - a.pos.x).abs().max((b.pos.y - a.pos.y).abs()).ceil() as usize).max(1);
        // Which of those samples can land in this band. `y` runs monotonically
        // along the segment, so they are one run, and skipping the rest is what
        // keeps a grid line that crosses the whole frame from being stepped end
        // to end once per band.
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
