//! The last frame's face depth, read back from the card to answer visibility and
//! surface-under-pointer queries.
//!
//! Replaces ray casts against a BVH rebuilt after every evaluation. The depth is copied after
//! faces and caps but before lines, into a pixel buffer read only when asked; the copy is only
//! made while queries are coming in. Answers match the picture: cut material is gone, dragged
//! bodies are where they were drawn, ghosts hide nothing.

use super::*;
use eframe::glow::{self, HasContext};
use std::cell::{Cell, RefCell};

/// One frame's depth, as the interface queries it.
pub(crate) struct DepthImage {
    /// The frame it was copied from.
    frame: u64,
    width: usize,
    height: usize,
    /// One window depth per pixel, rows from the top of the picture.
    depth: Vec<f32>,
    /// The frame's depth mapping (see `draw`), to turn depths back into keys.
    offset: f32,
    scale: f32,
    /// The picture's top-left in interface coordinates, and pixels per point.
    origin: egui::Pos2,
    pixels_per_point: f32,
}

/// The read-back machinery, held by the `Gpu`.
pub(crate) struct DepthReadback {
    buffer: glow::Buffer,
    /// Whether a query came since the last frame, so the next one copies its depth.
    wanted: Cell<bool>,
    /// A copy in flight, with what is needed to read it.
    pending: Cell<Option<Pending>>,
    image: RefCell<Option<DepthImage>>,
    /// Where the panel is, for the frame being drawn.
    placement: Cell<(egui::Pos2, f32)>,
    /// Frames drawn so far; a copy answers only for its own frame.
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
    /// The face key at an interface point: `Some(None)` for no face, `None` off the picture.
    ///
    /// Interpolated between the four surrounding pixel centres when they are coplanar (a flat face),
    /// otherwise the containing pixel's, since interpolating across an edge lands in mid-air.
    fn key_at(&self, screen: egui::Pos2) -> Option<Option<f32>> {
        self.key_between(screen).map(|key| key.map(|(key, _)| key))
    }

    /// [`DepthImage::key_at`], plus whether it was interpolated (exact on a flat face).
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

    /// The key of the pixel an interface point falls in.
    fn pixel_key_at(&self, screen: egui::Pos2) -> Option<Option<f32>> {
        let x = (screen.x - self.origin.x) * self.pixels_per_point;
        let y = (screen.y - self.origin.y) * self.pixels_per_point;
        if x < 0.0 || y < 0.0 {
            return None;
        }
        self.pixel_key(x as usize, y as usize)
    }

    /// One pixel's key: `Some(None)` for no face, `None` off the picture.
    fn pixel_key(&self, x: usize, y: usize) -> Option<Option<f32>> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let z = self.depth[y * self.width + x];
        Some((z < 1.0).then(|| (self.offset - (2.0 * z - 1.0)) / self.scale))
    }

    /// The key change across one pixel, measured to the neighbour nearest in key (the same face
    /// beside an edge).
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

    /// The key distance between neighbouring depth buffer steps.
    fn step(&self) -> f32 {
        2.0 / (self.scale * (1 << 24) as f32)
    }
}

impl Gpu {
    /// Set where the next frame's panel sits.
    pub fn place(&self, origin: egui::Pos2, pixels_per_point: f32) {
        self.depth.placement.set((origin, pixels_per_point));
    }

    /// Copy face depth into the pixel buffer if a query came since the last frame. Expects the
    /// model pass's framebuffer bound.
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

    /// The last frame's face key at an interface point: `Some(None)` for no face, `None` if not read
    /// back yet (the caller falls back this once and the next frame is copied).
    pub fn surface_key(&self, screen: egui::Pos2) -> Option<Option<f32>> {
        self.depth.wanted.set(true);
        if let Some(pending) = self.depth.pending.take() {
            let image = unsafe { self.read_depth(pending) };
            *self.depth.image.borrow_mut() = image;
        }
        let image = self.depth.image.borrow();
        image.as_ref().filter(|image| image.frame == self.depth.frame.get())?.key_at(screen)
    }

    /// Whether a query went unanswered for lack of a copy, so the viewport redraws and copies.
    pub fn depth_wanted(&self) -> bool {
        let frame = self.depth.frame.get();
        let fresh = self.depth.pending.get().is_some_and(|pending| pending.frame == frame)
            || self.depth.image.borrow().as_ref().is_some_and(|image| image.frame == frame);
        self.depth.wanted.get() && !fresh
    }

    /// Whether the last frame shows a point with depth key `key` at an interface point, or a face
    /// hides it; `None` if unknown (off the picture or not read back).
    ///
    /// Shown when nothing nearer is drawn there, with a slope tolerance across edges; a point on an
    /// edge is also shown when a neighbouring pixel has its face.
    pub fn in_sight(&self, screen: egui::Pos2, key: f32) -> Option<bool> {
        self.surface_key(screen)?;
        let image = self.depth.image.borrow();
        let image = image.as_ref()?;
        let Some((surface, exact)) = image.key_between(screen)? else { return Some(true) };
        // A hundredth of a millimetre (as the ray cast allowed), a few depth steps, and the pixel's
        // slope when not interpolated.
        let slope = if exact { 0.0 } else { image.slope_at(screen) };
        let tolerance = 1e-2 + 4.0 * image.step();
        if surface <= key + tolerance + slope {
            return Some(true);
        }
        // A point on an edge may fall in the neighbouring face's pixel, so any of the four neighbours
        // showing a face no nearer than the point counts.
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
        // Rows come back bottom-up, and the framebuffer's bottom is the picture's top (see
        // `VERTEX_SOURCE`), so they are already in picture order.
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
