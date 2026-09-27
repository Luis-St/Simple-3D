//! The frame as rows, and the band one thread owns.

use super::*;
impl Frame<'_> {
    /// Fill with one colour and reset depth; only the rasterizer's tests use it now.
    #[cfg(test)]
    pub fn clear(&mut self, background: Rgba) {
        for pixel in self.color.chunks_exact_mut(4) {
            pixel.copy_from_slice(&background);
        }
        for key in &mut self.key {
            *key = f32::NEG_INFINITY;
        }
    }

    #[cfg(test)]
    pub fn new(color: &mut [u8], width: usize, height: usize) -> Frame<'_> {
        Frame::band(color, width, height, 0, height)
    }

    /// A frame of rows `[row_lo, row_hi)` only, clipped to them and sized for them, so N bands cost one frame.
    pub fn band(color: &mut [u8], width: usize, height: usize, row_lo: usize, row_hi: usize) -> Frame<'_> {
        let rows = row_hi.saturating_sub(row_lo);
        debug_assert_eq!(color.len(), width * rows * 4, "the slice is not this band's rows");
        Frame {
            width,
            height,
            row_lo,
            row_hi,
            color,
            key: vec![f32::NEG_INFINITY; width * rows],
            owner: vec![0; width * rows],
            tag: 0,
        }
    }

    pub fn rows(&self) -> std::ops::Range<usize> {
        self.row_lo..self.row_hi
    }
}
