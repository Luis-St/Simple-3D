//! Mesh data laid out in textures, for shader stages that look entries up by index.

use super::*;
use std::ops::Range;

/// How a table's texels are laid out.
#[derive(Clone, Copy)]
pub(in crate::gpu) enum Format {
    Vec3,
    Vec4,
    UVec3,
    U16,
    U32,
}

impl Format {
    /// Internal format, pixel format, component type and bytes per texel.
    fn gl(self) -> (u32, u32, u32, usize) {
        match self {
            Format::Vec3 => (glow::RGB32F, glow::RGB, glow::FLOAT, 12),
            Format::Vec4 => (glow::RGBA32F, glow::RGBA, glow::FLOAT, 16),
            Format::UVec3 => (glow::RGB32UI, glow::RGB_INTEGER, glow::UNSIGNED_INT, 12),
            Format::U16 => (glow::R16UI, glow::RED_INTEGER, glow::UNSIGNED_SHORT, 2),
            Format::U32 => (glow::R32UI, glow::RED_INTEGER, glow::UNSIGNED_INT, 4),
        }
    }
}

/// `count` texels as a texture `width` texels wide, filled row by row. The short last row is
/// uploaded separately so nothing needs padding.
pub(in crate::gpu) unsafe fn table(
    gl: &glow::Context,
    width: usize,
    format: Format,
    bytes: &[u8],
    count: usize,
) -> Result<glow::Texture, String> {
    let (internal, pixels, kind, texel) = format.gl();
    let rows = count.div_ceil(width).max(1);
    let texture = gl.create_texture()?;
    gl.bind_texture(glow::TEXTURE_2D, Some(texture));
    clamp_texture(gl, glow::NEAREST, glow::NEAREST);
    gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
    let upload_width = if rows == 1 { count.max(1) } else { width };
    gl.tex_image_2d(
        glow::TEXTURE_2D,
        0,
        internal as i32,
        upload_width as i32,
        rows as i32,
        0,
        pixels,
        kind,
        glow::PixelUnpackData::Slice(None),
    );
    let full = count / upload_width;
    if full > 0 {
        let data = &bytes[..full * upload_width * texel];
        let unpack = glow::PixelUnpackData::Slice(Some(data));
        gl.tex_sub_image_2d(glow::TEXTURE_2D, 0, 0, 0, upload_width as i32, full as i32, pixels, kind, unpack);
    }
    let rest = count - full * upload_width;
    if rest > 0 {
        let data = &bytes[full * upload_width * texel..count * texel];
        let unpack = glow::PixelUnpackData::Slice(Some(data));
        gl.tex_sub_image_2d(glow::TEXTURE_2D, 0, 0, full as i32, rest as i32, 1, pixels, kind, unpack);
    }
    gl.bind_texture(glow::TEXTURE_2D, None);
    Ok(texture)
}

/// Overwrite texels `range` of a table made by [`table`] for `total` texels, from `bytes`. Laid out
/// as `table` lays it out: one row when it fits, else rows `width` wide.
pub(super) unsafe fn retable(
    gl: &glow::Context,
    texture: glow::Texture,
    width: usize,
    format: Format,
    total: usize,
    range: &Range<u32>,
    bytes: &[u8],
) {
    let (_, pixels, kind, texel) = format.gl();
    let wide = if total.div_ceil(width) <= 1 { total.max(1) } else { width };
    gl.bind_texture(glow::TEXTURE_2D, Some(texture));
    gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
    let (mut at, end) = (range.start as usize, range.end as usize);
    while at < end {
        let (row, column) = (at / wide, at % wide);
        // Whole rows in one call, the ragged ends one row each.
        let (texels, size) = match column == 0 && end - at >= wide {
            true => {
                let rows = (end - at) / wide;
                (rows * wide, (wide, rows))
            }
            false => {
                let take = (wide - column).min(end - at);
                (take, (take, 1))
            }
        };
        let from = (at - range.start as usize) * texel;
        let data = glow::PixelUnpackData::Slice(Some(&bytes[from..from + texels * texel]));
        let (x, y, w, h) = (column as i32, row as i32, size.0 as i32, size.1 as i32);
        gl.tex_sub_image_2d(glow::TEXTURE_2D, 0, x, y, w, h, pixels, kind, data);
        at += texels;
    }
    gl.bind_texture(glow::TEXTURE_2D, None);
}
