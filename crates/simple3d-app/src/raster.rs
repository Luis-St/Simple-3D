//! A small software rasterizer with a depth buffer.
//!
//! The viewport is drawn on the CPU rather than through the GPU. That is a
//! deliberate response to two hard constraints: the preview must work on
//! integrated graphics and the application must "degrade gracefully rather than
//! refuse to start if accelerated rendering is unavailable" (spec section 2.7,
//! acceptance criterion 19). A software path has no shader compilation, no
//! driver feature detection and no fallback to write -- it simply always works,
//! and the window itself is the only thing that needs a graphics backend.
//!
//! Depth is stored as a *key* that is linear in screen space and larger always
//! means nearer. The projection is orthographic, so the key is simply `-z`:
//! view-space depth interpolates linearly across a triangle on screen, and
//! nothing has to be clipped against a near plane to keep it doing so.

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
    /// The rows this frame actually holds, `[row_lo, row_hi)` of a frame
    /// `height` rows tall. A whole frame owns all of them; a *band* owns a
    /// horizontal stripe and nothing else, which is how the rasterizer is
    /// split across threads.
    ///
    /// Banding is what makes the parallel path produce the very same picture
    /// as the single-threaded one rather than merely a similar one. Every band
    /// replays the entire draw sequence and simply drops the writes outside its
    /// own rows, so each pixel is still written by the same primitives, in the
    /// same order, with the same depth decisions. Splitting the *work* instead
    /// -- a thread per item, say -- would reorder the writes that land on a
    /// shared pixel, and the picture would depend on how the threads raced.
    row_lo: usize,
    row_hi: usize,
    /// This band's rows of the image being built, RGBA and row-major. Borrowed
    /// rather than owned: every band writes straight into its own stretch of
    /// the one buffer that becomes the texture, so there is no gathering pass
    /// at the end. Copying the bands together afterwards cost more than the
    /// drawing did on a large viewport.
    pub color: &'a mut [u8],
    key: Vec<f32>,
    /// Which item owns the depth at each pixel: `tag` at the time it was
    /// written, and 0 for a pixel no solid has claimed. What lets a line ask
    /// *what* is in front of it rather than only whether something is -- an
    /// origin axis is not hidden by the solid it runs through, and is hidden by
    /// every other one.
    owner: Vec<u16>,
    tag: u16,
}

/// A finished frame: the colour buffer the bands wrote between them.
pub struct Image {
    pub width: usize,
    pub height: usize,
    /// RGBA, row-major from the top left -- the layout `egui::ColorImage` wants.
    pub color: Vec<u8>,
}

impl Image {
    /// Hand the framebuffer to egui as a texture image, without copying it.
    ///
    /// Every pixel the renderer writes is opaque -- the background is laid
    /// down opaque, a face is either copied in at full alpha or blended with
    /// the result's alpha set to full -- and an opaque pixel reads the same
    /// four bytes whether its alpha is taken as straight or premultiplied. So
    /// the buffer already is what egui wants, and converting it pixel by pixel
    /// was a pass over the whole frame, every frame, for nothing.
    pub fn into_color_image(self) -> egui::ColorImage {
        debug_assert!(self.color.chunks_exact(4).all(|pixel| pixel[3] == 255), "the frame has a see-through pixel");
        let size = [self.width, self.height];
        let mut color = std::mem::ManuallyDrop::new(self.color);
        let (bytes, len, capacity) = (color.as_mut_ptr(), color.len(), color.capacity());
        if len % 4 != 0 || capacity % 4 != 0 {
            // Not a whole number of pixels to hand over: copy instead.
            let color = std::mem::ManuallyDrop::into_inner(color);
            return egui::ColorImage::from_rgba_premultiplied(size, &color);
        }
        // `Color32` is `repr(C)` over `[u8; 4]`, so it has the bytes' own
        // alignment, and a buffer of `4n` bytes is exactly a buffer of `n` of
        // them -- the allocation is released with the layout it was made with.
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
