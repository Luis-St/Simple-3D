//! Putting one pixel down, with depth and with blending.

use super::*;
impl Frame<'_> {
    /// Whose depth writes are from here on. The renderer sets it to the item it
    /// is about to draw, and back to 0 for anything that belongs to no item.
    pub fn set_tag(&mut self, tag: u16) {
        self.tag = tag;
    }

    /// Depth-tested, optionally alpha-blended write. `write_depth` is false for
    /// translucent passes, so ghosts do not hide each other.
    pub(super) fn put(&mut self, x: usize, y: usize, key: f32, rgba: Rgba, write_depth: bool) {
        self.put_with(x, y, key, rgba, write_depth, None);
    }

    /// As `put`, with a list of the items this write may be drawn through,
    /// indexed by their tag. A pixel that loses the depth test is still written
    /// if what won it is one of them -- which is how an axis crosses the solid
    /// it runs into without crossing the ones it merely passes behind.
    pub(super) fn put_with(
        &mut self,
        x: usize,
        y: usize,
        key: f32,
        rgba: Rgba,
        write_depth: bool,
        through: Option<&[bool]>,
    ) {
        // A row outside this band belongs to another one, which is drawing it
        // from the very same sequence of primitives.
        if y < self.row_lo || y >= self.row_hi {
            return;
        }
        let i = (y - self.row_lo) * self.width + x;
        if key <= self.key[i] {
            let seen = through.is_some_and(|items| items.get(self.owner[i] as usize).copied().unwrap_or(false));
            if !seen {
                return;
            }
        }
        let o = i * 4;
        if rgba[3] == 255 {
            self.color[o..o + 4].copy_from_slice(&rgba);
        } else {
            self.blend(o, rgba);
        }
        if write_depth {
            self.key[i] = key;
            self.owner[i] = self.tag;
        }
    }

    /// Blend over a pixel whatever its depth, and leave the depth alone. The
    /// glow that says a body is there when something else is in front of it.
    pub(super) fn put_over(&mut self, x: usize, y: usize, rgba: Rgba) {
        if y < self.row_lo || y >= self.row_hi {
            return;
        }
        self.blend(((y - self.row_lo) * self.width + x) * 4, rgba);
    }

    /// Straight-alpha blend of one colour over the pixel at byte offset `o`.
    pub(super) fn blend(&mut self, o: usize, rgba: Rgba) {
        let a = rgba[3] as u32;
        for (c, &value) in rgba.iter().enumerate().take(3) {
            let src = value as u32 * a;
            let dst = self.color[o + c] as u32 * (255 - a);
            self.color[o + c] = ((src + dst) / 255) as u8;
        }
        self.color[o + 3] = 255;
    }
}
