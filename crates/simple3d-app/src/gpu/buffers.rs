//! Uploading geometry, resizing, and handing the texture back.

use super::*;
use crate::raster::Rgba;
use crate::render::AxisStep;
use eframe::glow::{self, HasContext};

impl Gpu {
    pub(super) unsafe fn batch(&self, gl: &glow::Context, mode: u32, vertices: &[GpuVertex]) {
        if vertices.is_empty() {
            return;
        }
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.buffer.vertices));
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, as_bytes(vertices), glow::STREAM_DRAW);
        gl.draw_arrays(mode, 0, vertices.len() as i32);
    }

    /// The table the axis shader reads: one row per segment of an axis, one
    /// column per body tag, holding whether that segment may be seen through
    /// that body.
    pub(super) unsafe fn upload_seen(&self, gl: &glow::Context, axes: &[AxisStep], tags: usize) {
        let mut table = vec![0u8; tags * axes.len().max(1)];
        for (segment, step) in axes.iter().enumerate() {
            for (tag, &allowed) in step.seen.iter().enumerate() {
                if allowed && tag < tags {
                    table[segment * tags + tag] = 255;
                }
            }
        }
        gl.bind_texture(glow::TEXTURE_2D, Some(self.buffer.seen));
        gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::R8 as i32,
            tags as i32,
            axes.len().max(1) as i32,
            0,
            glow::RED,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(Some(&table)),
        );
    }

    /// Make or remake the offscreen target. The colour texture keeps its
    /// identity across a resize so egui's texture id stays good.
    pub(super) unsafe fn resize(&mut self, gl: &glow::Context, width: usize, height: usize) -> Result<(), String> {
        if let Some(target) = &self.target {
            if target.width == width && target.height == height {
                return Ok(());
            }
        }
        let colour = match self.colour {
            Some(colour) => colour,
            None => {
                let colour = gl.create_texture()?;
                self.colour = Some(colour);
                colour
            }
        };
        gl.bind_texture(glow::TEXTURE_2D, Some(colour));
        for (name, value) in [
            (glow::TEXTURE_MIN_FILTER, glow::LINEAR),
            (glow::TEXTURE_MAG_FILTER, glow::LINEAR),
            (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
            (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
        ] {
            gl.tex_parameter_i32(glow::TEXTURE_2D, name, value as i32);
        }
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA8 as i32,
            width as i32,
            height as i32,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(None),
        );
        if let Some(old) = self.target.take() {
            old.destroy(gl);
        }
        self.target = Some(Target::new(gl, colour, width, height)?);
        Ok(())
    }

    /// Make the colour texture, so egui has something to be given. The first
    /// real frame reallocates it to the viewport's size; the texture object --
    /// and so egui's id for it -- stays the same.
    pub fn prepare_texture(&mut self) -> Result<glow::Texture, String> {
        let gl = self.gl.clone();
        unsafe { self.resize(&gl, 1, 1)? };
        self.colour.ok_or_else(|| "the colour texture was not made".to_string())
    }

    pub fn set_texture_id(&mut self, id: egui::TextureId) {
        self.texture_id = Some(id);
    }
}

pub(crate) fn as_float(colour: Rgba) -> [f32; 4] {
    [colour[0] as f32 / 255.0, colour[1] as f32 / 255.0, colour[2] as f32 / 255.0, colour[3] as f32 / 255.0]
}

pub(crate) fn as_bytes(vertices: &[GpuVertex]) -> &[u8] {
    // `GpuVertex` is `repr(C)` and holds only numbers, so its bytes are what
    // OpenGL is being handed.
    unsafe { std::slice::from_raw_parts(vertices.as_ptr() as *const u8, std::mem::size_of_val(vertices)) }
}
