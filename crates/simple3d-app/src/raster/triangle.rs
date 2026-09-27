//! Filling a triangle.

use super::*;
impl Frame<'_> {
    /// Fill a screen-space triangle in either winding; culling is the caller's decision in world space.
    pub fn triangle(&mut self, v: [Vertex; 3], rgba: Rgba, write_depth: bool) {
        self.triangle_inner(v, rgba, write_depth, true);
    }

    /// A triangle drawn over everything with depth ignored both ways, for a body buried inside another.
    pub fn triangle_over(&mut self, v: [Vertex; 3], rgba: Rgba) {
        self.triangle_inner(v, rgba, false, false);
    }

    pub(super) fn triangle_inner(&mut self, v: [Vertex; 3], rgba: Rgba, write_depth: bool, test_depth: bool) {
        let (p0, p1, p2) = (v[0].pos, v[1].pos, v[2].pos);
        let area = edge(p0, p1, p2);
        if area.abs() < 1e-9 {
            return;
        }
        // A consistent winding, so the inside test is a single sign.
        let (v, area) = if area < 0.0 { ([v[0], v[2], v[1]], -area) } else { (v, area) };
        let (p0, p1, p2) = (v[0].pos, v[1].pos, v[2].pos);

        // Rounded without `floor`/`ceil`, which are libm calls on baseline x86-64 and cost an eighth of the
        // fill time (see `floor_at_zero`, `ceil_of`).
        let min_x = floor_at_zero(p0.x.min(p1.x).min(p2.x));
        let max_x = ceil_of(p0.x.max(p1.x).max(p2.x)).clamp(0, self.width as isize - 1) as usize;
        let min_y = floor_at_zero(p0.y.min(p1.y).min(p2.y));
        let max_y = ceil_of(p0.y.max(p1.y).max(p2.y)).clamp(0, self.height as isize - 1) as usize;
        // Rows outside this band are another band's work.
        let min_y = min_y.max(self.row_lo);
        let max_y = max_y.min(self.row_hi.saturating_sub(1));
        if min_x > max_x || min_y > max_y {
            return;
        }

        // Per-pixel edge-function slopes, used only to find each row's span; the exact test still runs per
        // pixel, so the picture is unchanged.
        let slope = [-(p2.y - p1.y), -(p0.y - p2.y), -(p1.y - p0.y)];

        let inv_area = 1.0 / area;
        for y in min_y..=max_y {
            let start = egui::pos2(min_x as f32 + 0.5, y as f32 + 0.5);
            let at_start = [edge(p1, p2, start), edge(p2, p0, start), edge(p0, p1, start)];
            // The row's span where the three half-planes meet, so thin triangles do not test their whole box.
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
            // A pixel of slack each end, so rounding never shortens the exact test's run.
            let from = floor_at_zero(from.max(min_x as f32)).saturating_sub(1).max(min_x);
            let to = (ceil_of(to).max(0) as usize + 1).min(max_x);
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

/// `v.floor().max(0.0) as usize` without calling `floor`: truncation floors non-negative values,
/// and negatives and NaN become zero either way.
fn floor_at_zero(v: f32) -> usize {
    v.max(0.0) as usize
}

/// `v.ceil() as isize` without calling `ceil`: truncate and step up if short; same saturation and NaN.
fn ceil_of(v: f32) -> isize {
    let truncated = v as isize;
    if (truncated as f32) < v {
        truncated + 1
    } else {
        truncated
    }
}

pub(crate) fn edge(a: egui::Pos2, b: egui::Pos2, c: egui::Pos2) -> f32 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}
