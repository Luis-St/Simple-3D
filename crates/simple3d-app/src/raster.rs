//! A small software rasterizer with a depth buffer.
//!
//! Works everywhere without GPU features, so the viewport degrades gracefully when acceleration
//! is unavailable (spec section 2.7, acceptance criterion 19). Depth is stored as a key, larger is
//! nearer; with an orthographic projection it is simply `-z`, linear on screen with no near clip.

mod bands;
mod clip;
mod line;
mod pixel;
#[cfg(test)]
mod tests;
mod triangle;

/// Straight-alpha RGBA.
pub type Rgba = [u8; 4];

pub struct Frame<'a> {
    pub width: usize,
    pub height: usize,
    /// The rows this frame holds, `[row_lo, row_hi)` of `height`; a band owns one stripe. Every band
    /// replays the whole draw sequence and drops writes outside its rows, so the parallel picture is
    /// identical to the single-threaded one, whereas splitting by item would race on shared pixels.
    row_lo: usize,
    row_hi: usize,
    /// This band's rows of the output, RGBA row-major, borrowed so bands write straight into the final
    /// buffer; gathering them afterwards cost more than drawing.
    pub color: &'a mut [u8],
    key: Vec<f32>,
    /// Which item owns each pixel's depth (`tag` when written, 0 if unclaimed), so an axis can be
    /// hidden by other solids but not by the one it runs through.
    owner: Vec<u16>,
    tag: u16,
}

/// A finished frame: the colour buffer the bands wrote.
pub struct Image {
    pub width: usize,
    pub height: usize,
    /// RGBA, row-major from the top left, as `egui::ColorImage` wants.
    pub color: Vec<u8>,
}

impl Image {
    /// Hand the framebuffer to egui without copying. Every written pixel is opaque, so straight and
    /// premultiplied alpha coincide and no per-pixel conversion is needed.
    pub fn into_color_image(self) -> egui::ColorImage {
        debug_assert!(self.color.chunks_exact(4).all(|pixel| pixel[3] == 255), "the frame has a see-through pixel");
        let size = [self.width, self.height];
        let mut color = std::mem::ManuallyDrop::new(self.color);
        let (bytes, len, capacity) = (color.as_mut_ptr(), color.len(), color.capacity());
        if len % 4 != 0 || capacity % 4 != 0 {
            // Not a whole number of pixels: copy instead.
            let color = std::mem::ManuallyDrop::into_inner(color);
            return egui::ColorImage::from_rgba_premultiplied(size, &color);
        }
        // `Color32` is `repr(C)` over `[u8; 4]` with the same alignment, so `4n` bytes are exactly `n`
        // colours and the allocation is freed with its original layout.
        let pixels = unsafe { Vec::from_raw_parts(bytes as *mut egui::Color32, len / 4, capacity / 4) };
        egui::ColorImage::new(size, pixels)
    }
}

/// One projected vertex: screen position and depth key.
#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub pos: egui::Pos2,
    pub key: f32,
}
