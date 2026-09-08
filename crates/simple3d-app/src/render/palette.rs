//! The colours a frame is drawn in.

use crate::raster::{Frame, Rgba};

/// Colours, resolved from the host theme so the viewport is usable under both
/// light and dark system themes (spec section 7.4).
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    /// The top of the viewport's vertical gradient.
    pub background: Rgba,
    /// The bottom of it. A flat field of one colour reads as a blank canvas;
    /// a gradient this shallow is barely nameable but gives the ground plane
    /// somewhere to sit.
    pub background_low: Rgba,
    pub solid: Rgba,
    pub selected: Rgba,
    pub ghost: Rgba,
    pub grid: Rgba,
    pub grid_major: Rgba,
    pub axis_x: Rgba,
    pub axis_y: Rgba,
    pub axis_z: Rgba,
    pub wire: Rgba,
    pub edge: Rgba,
    /// The fill a body glowing through another one is drawn in: the selection
    /// colour, translucent enough that the shape in front of it still reads.
    pub glow: Rgba,
    /// The face a section leaves behind (issue 71): the inside of the material,
    /// which is the one surface in the frame that is not a surface of the
    /// model. Darker than a solid on purpose -- a cut that reads in the same
    /// colour as the outside says nothing about where the wall ends.
    pub cut: Rgba,
}

impl Palette {
    /// The viewport's own reading of the interface palette. The names on the
    /// left are `crate::theme::token`'s: surface-0 for the ground, the amber
    /// accent for selection, the danger red for a body that is being subtracted.
    pub fn dark() -> Palette {
        use crate::theme::token;
        Palette {
            background: rgba(token::SURFACE_0),
            background_low: rgba(token::SURFACE_0B),
            solid: [0x9A, 0xA4, 0xB2, 255],
            selected: rgba(token::ACCENT),
            // A subtrahend is drawn as a translucent red ghost, so a cut can be
            // seen before it is resolved.
            ghost: fade(token::DANGER, 80),
            grid: [0x28, 0x2D, 0x35, 255],
            grid_major: rgba(token::SURFACE_3),
            axis_x: rgba(token::AXIS_X),
            axis_y: rgba(token::AXIS_Y),
            axis_z: rgba(token::AXIS_Z),
            wire: [0xC2, 0xCA, 0xD6, 255],
            edge: [0x11, 0x13, 0x17, 255],
            glow: fade(token::ACCENT, 110),
            cut: [0x5E, 0x66, 0x73, 255],
        }
    }

    pub fn light() -> Palette {
        Palette {
            background: [238, 240, 243, 255],
            background_low: [226, 229, 234, 255],
            solid: [150, 158, 170, 255],
            selected: [226, 122, 12, 255],
            ghost: [40, 110, 220, 60],
            grid: [214, 218, 224, 255],
            grid_major: [186, 192, 200, 255],
            axis_x: [186, 54, 54, 255],
            axis_y: [50, 140, 50, 255],
            axis_z: [46, 96, 200, 255],
            wire: [64, 70, 80, 255],
            edge: [70, 76, 86, 255],
            glow: [226, 122, 12, 120],
            cut: [104, 112, 124, 255],
        }
    }

    pub fn for_dark_mode(dark: bool) -> Palette {
        if dark {
            Palette::dark()
        } else {
            Palette::light()
        }
    }
}

/// A palette token as the rasterizer's own pixel format.
pub(crate) fn rgba(colour: egui::Color32) -> Rgba {
    [colour.r(), colour.g(), colour.b(), 255]
}

/// The same, at a chosen alpha.
pub(crate) fn fade(colour: egui::Color32, alpha: u8) -> Rgba {
    [colour.r(), colour.g(), colour.b(), alpha]
}

impl Palette {
    /// The background colour at `row` of a frame `height` rows tall. One
    /// definition, used by the renderer and by anything that needs to ask
    /// "was this pixel painted, or is it just the sky".
    pub fn background_at(&self, row: usize, height: usize) -> Rgba {
        if height <= 1 {
            return self.background;
        }
        let t = row as f32 / (height - 1) as f32;
        let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        [
            mix(self.background[0], self.background_low[0]),
            mix(self.background[1], self.background_low[1]),
            mix(self.background[2], self.background_low[2]),
            255,
        ]
    }
}

/// Lay the gradient down one row at a time, before anything else is drawn.
pub(crate) fn fill_background(frame: &mut Frame, palette: &Palette) {
    // The gradient is a property of the whole frame, so the colour is asked for
    // by the row's place in it -- while the pixels written are this band's own.
    let height = frame.height;
    let width = frame.width;
    let rows = frame.rows();
    for (offset, row) in rows.enumerate() {
        let colour = palette.background_at(row, height);
        let line = &mut frame.color[offset * width * 4..(offset + 1) * width * 4];
        for pixel in line.chunks_exact_mut(4) {
            pixel.copy_from_slice(&colour);
        }
    }
}
