//! What the last frame drew where: the depth of the model's faces, read back
//! from the card for the questions the interface asks about the picture.
//!
//! "Can this point be seen?" and "what surface is under the pointer?" were
//! answered by casting a ray through the whole evaluated scene, which needs a
//! bounding volume hierarchy of every triangle in it -- built again, on the
//! interface thread, after every evaluation. The frame the card has just drawn
//! already knows the answer for every pixel. Its depth is copied, once the
//! faces and the section's cap are down and before any line is drawn over
//! them, into a pixel buffer the card fills while it goes on drawing, and read
//! out only when a question is asked: a frame nobody asks about costs nothing
//! beyond the copy, and the copy is only made while somebody is asking.
//!
//! The answers are the picture's: a section's cut-away material is gone, a
//! dragged body is where the drag has got to, and a ghost hides nothing.

use super::*;
use eframe::glow::{self, HasContext};
use std::cell::{Cell, RefCell};

/// One frame's depth, as the interface asks about it.
pub(crate) struct DepthImage {
    /// The frame it was copied from.
    frame: u64,
    width: usize,
    height: usize,
    /// One window depth per pixel, rows from the top of the picture.
    depth: Vec<f32>,
    /// The frame's depth mapping -- see `draw` -- to turn a depth back into a
    /// key.
    offset: f32,
    scale: f32,
    /// Where the picture's top-left corner is in the interface, and how many
    /// pixels a point is.
    origin: egui::Pos2,
    pixels_per_point: f32,
}

/// The read-back machinery, held by the `Gpu`.
pub(crate) struct DepthReadback {
    buffer: glow::Buffer,
    /// Whether a question was asked since the last frame, so the next one
    /// copies its depth.
    wanted: Cell<bool>,
    /// A copy on its way, with what is needed to read it.
    pending: Cell<Option<Pending>>,
    image: RefCell<Option<DepthImage>>,
    /// Where the panel is, for the frame being drawn.
    placement: Cell<(egui::Pos2, f32)>,
    /// How many frames have been drawn: a copy answers for the frame it was
    /// made from and no other.
    frame: Cell<u64>,
}

#[derive(Clone, Copy)]
struct Pending {
    frame: u64,
    width: usize,
    height: usize,
    offset: f32,
    scale: f32,
    origin: egui::Pos2,
    pixels_per_point: f32,
}

impl DepthReadback {
    pub(super) unsafe fn new(gl: &glow::Context) -> Result<DepthReadback, String> {
        Ok(DepthReadback {
            buffer: gl.create_buffer()?,
            wanted: Cell::new(false),
            pending: Cell::new(None),
            image: RefCell::new(None),
            placement: Cell::new((egui::Pos2::ZERO, 1.0)),
            frame: Cell::new(0),
        })
    }
}

impl DepthImage {
    /// The key of the face drawn at a point of the interface: `Some(None)` for
    /// a pixel no face covers, `None` off the picture.
    ///
    /// Read between the four pixel centres round the point where they lie on
    /// one plane -- a flat face, which the depth is linear across -- and off
    /// the pixel the point is in where they do not: across an edge or an
    /// outline, which interpolating would put a point in mid-air.
    fn key_at(&self, screen: egui::Pos2) -> Option<Option<f32>> {
        self.key_between(screen).map(|key| key.map(|(key, _)| key))
    }

    /// [`DepthImage::key_at`], with whether it was read between pixel centres
    /// -- exact on a flat face -- or off the one pixel.
    fn key_between(&self, screen: egui::Pos2) -> Option<Option<(f32, bool)>> {
        let x = (screen.x - self.origin.x) * self.pixels_per_point;
        let y = (screen.y - self.origin.y) * self.pixels_per_point;
        if x < 0.0 || y < 0.0 || x as usize >= self.width || y as usize >= self.height {
            return None;
        }
        let nearest = self.pixel_key(x as usize, y as usize)?;
        let Some(nearest) = nearest else { return Some(None) };
        let (fx, fy) = (x - 0.5, y - 0.5);
        let (x0, y0) = (fx.floor(), fy.floor());
        if x0 < 0.0 || y0 < 0.0 {
            return Some(Some((nearest, false)));
        }
        let (x0, y0) = (x0 as usize, y0 as usize);
        let corners = [(0, 0), (1, 0), (0, 1), (1, 1)].map(|(dx, dy)| self.pixel_key(x0 + dx, y0 + dy).flatten());
        let [Some(k00), Some(k10), Some(k01), Some(k11)] = corners else { return Some(Some((nearest, false))) };
        // On one plane the two diagonals change by the same amount.
        let spread = (k10 - k00).abs().max((k01 - k00).abs()) + 1e-6;
        if ((k00 + k11) - (k10 + k01)).abs() > spread * 1e-2 + self.step() {
            return Some(Some((nearest, false)));
        }
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let top = k00 + (k10 - k00) * tx;
        let bottom = k01 + (k11 - k01) * tx;
        Some(Some((top + (bottom - top) * ty, true)))
    }

    /// The key of the one pixel a point of the interface falls in.
    fn pixel_key_at(&self, screen: egui::Pos2) -> Option<Option<f32>> {
        let x = (screen.x - self.origin.x) * self.pixels_per_point;
        let y = (screen.y - self.origin.y) * self.pixels_per_point;
        if x < 0.0 || y < 0.0 {
            return None;
        }
        self.pixel_key(x as usize, y as usize)
    }

    /// One pixel's key, `Some(None)` where no face is drawn and `None` off
    /// the picture.
    fn pixel_key(&self, x: usize, y: usize) -> Option<Option<f32>> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let z = self.depth[y * self.width + x];
        Some((z < 1.0).then(|| (self.offset - (2.0 * z - 1.0)) / self.scale))
    }

    /// How much the key changes across one pixel at a point, along x and y
    /// together: each measured to whichever neighbour is nearer in key, which
    /// is the one on the same face when the point is beside an edge.
    fn slope_at(&self, screen: egui::Pos2) -> f32 {
        let x = ((screen.x - self.origin.x) * self.pixels_per_point) as usize;
        let y = ((screen.y - self.origin.y) * self.pixels_per_point) as usize;
        let Some(Some(here)) = self.pixel_key(x, y) else { return 0.0 };
        let across = |a: Option<(usize, usize)>, b: Option<(usize, usize)>| {
            [a, b]
                .into_iter()
                .flatten()
                .filter_map(|(x, y)| self.pixel_key(x, y).flatten())
                .map(|k| (k - here).abs())
                .fold(f32::INFINITY, f32::min)
        };
        let dx = across(x.checked_sub(1).map(|x| (x, y)), Some((x + 1, y)));
        let dy = across(y.checked_sub(1).map(|y| (x, y)), Some((x, y + 1)));
        [dx, dy].into_iter().filter(|d| d.is_finite()).sum()
    }

    /// How far apart in key two neighbouring steps of the depth buffer are.
    fn step(&self) -> f32 {
        2.0 / (self.scale * (1 << 24) as f32)
    }
}

impl Gpu {
    /// Say where the panel the next frame is painted into sits.
    pub fn place(&self, origin: egui::Pos2, pixels_per_point: f32) {
        self.depth.placement.set((origin, pixels_per_point));
    }

    /// Copy the faces' depth into the pixel buffer, if a question was asked
    /// since the last frame. Called with the model pass's framebuffer bound.
    pub(super) unsafe fn copy_depth(&self, gl: &glow::Context, width: usize, height: usize, depth: [f32; 2]) {
        let frame = self.depth.frame.get() + 1;
        self.depth.frame.set(frame);
        if !self.depth.wanted.replace(false) {
            return;
        }
        let bytes = (width * height * 4) as i32;
        gl.bind_buffer(glow::PIXEL_PACK_BUFFER, Some(self.depth.buffer));
        gl.buffer_data_size(glow::PIXEL_PACK_BUFFER, bytes, glow::STREAM_READ);
        gl.pixel_store_i32(glow::PACK_ALIGNMENT, 4);
        gl.read_pixels(
            0,
            0,
            width as i32,
            height as i32,
            glow::DEPTH_COMPONENT,
            glow::FLOAT,
            glow::PixelPackData::BufferOffset(0),
        );
        gl.bind_buffer(glow::PIXEL_PACK_BUFFER, None);
        let (origin, pixels_per_point) = self.depth.placement.get();
        self.depth.pending.set(Some(Pending {
            frame,
            width,
            height,
            offset: depth[0],
            scale: depth[1],
            origin,
            pixels_per_point,
        }));
    }

    /// The key of the model's face drawn at a point of the interface in the
    /// last frame: `Some(None)` where no face is drawn, and `None` when the
    /// picture has not been read back yet -- the asker then answers the
    /// question some other way this once, and the next frame is copied.
    pub fn surface_key(&self, screen: egui::Pos2) -> Option<Option<f32>> {
        self.depth.wanted.set(true);
        if let Some(pending) = self.depth.pending.take() {
            let image = unsafe { self.read_depth(pending) };
            *self.depth.image.borrow_mut() = image;
        }
        let image = self.depth.image.borrow();
        image.as_ref().filter(|image| image.frame == self.depth.frame.get())?.key_at(screen)
    }

    /// Whether a question about the picture went unanswered because the frame
    /// on screen was never copied: the viewport then draws it again, copying
    /// it this time, rather than leave the asker to fall back for good.
    pub fn depth_wanted(&self) -> bool {
        let frame = self.depth.frame.get();
        let fresh = self.depth.pending.get().is_some_and(|pending| pending.frame == frame)
            || self.depth.image.borrow().as_ref().is_some_and(|image| image.frame == frame);
        self.depth.wanted.get() && !fresh
    }

    /// Whether the last frame shows a point whose depth key is `key` at a
    /// point of the interface, or a face stands in front of it: `None` when
    /// that cannot be said from the picture -- it is off it, or not read back
    /// yet.
    ///
    /// Shown when nothing is drawn at the point, or what is drawn there is no
    /// nearer than the point: read off the face at the point where it is flat
    /// across the pixels round it, and give or take the face's slope across a
    /// pixel where an edge runs between them. A point on an edge is also shown
    /// when a pixel beside it has its face.
    pub fn in_sight(&self, screen: egui::Pos2, key: f32) -> Option<bool> {
        self.surface_key(screen)?;
        let image = self.depth.image.borrow();
        let image = image.as_ref()?;
        let Some((surface, exact)) = image.key_between(screen)? else { return Some(true) };
        // A hundredth of a millimetre, as the ray cast allowed, a few of the
        // depth buffer's own steps, and -- where the depth is the one pixel's
        // rather than read off the face at the point -- the slope across it.
        let slope = if exact { 0.0 } else { image.slope_at(screen) };
        let tolerance = 1e-2 + 4.0 * image.step();
        if surface <= key + tolerance + slope {
            return Some(true);
        }
        // A point on an edge is drawn beside its own face, which the pixel it
        // falls in may not be: one of the four next to it showing a face no
        // nearer than the point is that face.
        let step = 1.0 / image.pixels_per_point;
        let beside = [(-step, 0.0), (step, 0.0), (0.0, -step), (0.0, step)];
        Some(beside.into_iter().any(
            |(dx, dy)| matches!(image.pixel_key_at(screen + egui::vec2(dx, dy)), Some(Some(k)) if k <= key + tolerance),
        ))
    }

    /// Read a finished copy out of the pixel buffer.
    unsafe fn read_depth(&self, pending: Pending) -> Option<DepthImage> {
        let gl = &self.gl;
        let count = pending.width * pending.height;
        gl.bind_buffer(glow::PIXEL_PACK_BUFFER, Some(self.depth.buffer));
        let mapped = gl.map_buffer_range(glow::PIXEL_PACK_BUFFER, 0, (count * 4) as i32, glow::MAP_READ_BIT);
        let depth = if mapped.is_null() {
            None
        } else {
            let values = std::slice::from_raw_parts(mapped as *const f32, count).to_vec();
            gl.unmap_buffer(glow::PIXEL_PACK_BUFFER);
            Some(values)
        };
        gl.bind_buffer(glow::PIXEL_PACK_BUFFER, None);
        // The rows came back from the bottom of the framebuffer up, and the
        // bottom of the framebuffer is the top of the picture (see
        // `VERTEX_SOURCE`), so they are in the picture's own order already.
        depth.map(|depth| DepthImage {
            frame: pending.frame,
            width: pending.width,
            height: pending.height,
            depth,
            offset: pending.offset,
            scale: pending.scale,
            origin: pending.origin,
            pixels_per_point: pending.pixels_per_point,
        })
    }
}
