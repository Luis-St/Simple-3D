//! Filling a triangle.

use super::*;
impl Frame<'_> {
    /// Fill a screen-space triangle. Vertices may be in either winding order;
    /// back-face culling is the caller's decision, made in world space where it
    /// is meaningful.
    pub fn triangle(&mut self, v: [Vertex; 3], rgba: Rgba, write_depth: bool) {
        self.triangle_inner(v, rgba, write_depth, true);
    }

    /// A triangle drawn over whatever is already there, depth ignored in both
    /// directions: it is not hidden by what is in front of it and it claims
    /// nothing of its own. What a body being pointed out *inside* another one
    /// is filled with, so it can be seen at all.
    pub fn triangle_over(&mut self, v: [Vertex; 3], rgba: Rgba) {
        self.triangle_inner(v, rgba, false, false);
    }

    pub(super) fn triangle_inner(&mut self, v: [Vertex; 3], rgba: Rgba, write_depth: bool, test_depth: bool) {
        let (p0, p1, p2) = (v[0].pos, v[1].pos, v[2].pos);
        let area = edge(p0, p1, p2);
        if area.abs() < 1e-9 {
            return;
        }
        // Work with a consistent winding so the inside test is a single sign.
        let (v, area) = if area < 0.0 { ([v[0], v[2], v[1]], -area) } else { (v, area) };
        let (p0, p1, p2) = (v[0].pos, v[1].pos, v[2].pos);

        let min_x = p0.x.min(p1.x).min(p2.x).floor().max(0.0) as usize;
        let max_x = (p0.x.max(p1.x).max(p2.x).ceil() as isize).clamp(0, self.width as isize - 1) as usize;
        let min_y = p0.y.min(p1.y).min(p2.y).floor().max(0.0) as usize;
        let max_y = (p0.y.max(p1.y).max(p2.y).ceil() as isize).clamp(0, self.height as isize - 1) as usize;
        // Rows outside this band are another band's work; not walking them at
        // all is the whole point of splitting the frame up.
        let min_y = min_y.max(self.row_lo);
        let max_y = max_y.min(self.row_hi.saturating_sub(1));
        if min_x > max_x || min_y > max_y {
            return;
        }

        // How each edge function changes from one pixel to the next along a
        // row. Used only to work out where the row enters and leaves the
        // triangle -- the test itself is still the exact one below, evaluated
        // per pixel, so the picture is the same to the last bit as when this
        // walked the whole bounding box.
        let slope = [-(p2.y - p1.y), -(p0.y - p2.y), -(p1.y - p0.y)];

        let inv_area = 1.0 / area;
        for y in min_y..=max_y {
            let start = egui::pos2(min_x as f32 + 0.5, y as f32 + 0.5);
            let at_start = [edge(p1, p2, start), edge(p2, p0, start), edge(p0, p1, start)];
            // The three half-planes meet in one run of pixels, this row's
            // span. A triangle thin against the pixel grid -- which most of a
            // curved surface's triangles are -- covers a few pixels of a
            // bounding box hundreds wide, and this is what stops the other
            // hundreds being tested one at a time.
            let (mut from, mut to) = (min_x as f32, max_x as f32);
            let mut empty = false;
            for i in 0..3 {
                if slope[i] > 0.0 {
                    from = from.max(min_x as f32 - at_start[i] / slope[i]);
                } else if slope[i] < 0.0 {
                    to = to.min(min_x as f32 - at_start[i] / slope[i]);
                } else if at_start[i] < 0.0 {
                    empty = true;
                }
            }
            if empty {
                continue;
            }
            // A pixel of slack at each end, so rounding in the division above
            // can never shorten the run the exact test would have accepted.
            let from = (from.floor().max(min_x as f32) as usize).saturating_sub(1).max(min_x);
            let to = ((to.ceil().max(0.0) as usize) + 1).min(max_x);
            for x in from..=to {
                let p = egui::pos2(x as f32 + 0.5, y as f32 + 0.5);
                let w0 = edge(p1, p2, p);
                let w1 = edge(p2, p0, p);
                let w2 = edge(p0, p1, p);
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let key = (w0 * v[0].key + w1 * v[1].key + w2 * v[2].key) * inv_area;
                if test_depth {
                    self.put(x, y, key, rgba, write_depth);
                } else {
                    self.put_over(x, y, rgba);
                }
            }
        }
    }
}

pub(crate) fn edge(a: egui::Pos2, b: egui::Pos2, c: egui::Pos2) -> f32 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}
