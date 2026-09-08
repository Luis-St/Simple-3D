//! The framebuffer drawn into, and its depth buffer.

use eframe::glow::{self, HasContext};

pub(crate) struct Target {
    pub(super) width: usize,
    pub(super) height: usize,
    /// Which body owns each pixel's depth, the counterpart of `Frame::owner`.
    pub(super) tags: glow::Texture,
    pub(super) depth: glow::Texture,
    /// Colour, tags and depth: what the model is drawn into.
    pub(super) scene: glow::Framebuffer,
    /// Colour alone, so the axis pass can *sample* the depth and tag textures
    /// that the scene pass wrote. A texture cannot be read and written in one
    /// pass, and the axis rule has to read both.
    pub(super) overlay: glow::Framebuffer,
}

impl Target {
    pub(super) unsafe fn new(
        gl: &glow::Context,
        colour: glow::Texture,
        width: usize,
        height: usize,
    ) -> Result<Target, String> {
        let plain = |texture: glow::Texture| {
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            for (name, value) in [
                (glow::TEXTURE_MIN_FILTER, glow::NEAREST),
                (glow::TEXTURE_MAG_FILTER, glow::LINEAR),
                (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
                (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
            ] {
                gl.tex_parameter_i32(glow::TEXTURE_2D, name, value as i32);
            }
        };
        let (w, h) = (width as i32, height as i32);

        // The tag buffer is exactly `Frame::owner`: one body number per pixel,
        // written only by the passes that write depth.
        let tags = gl.create_texture()?;
        plain(tags);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::R16UI as i32,
            w,
            h,
            0,
            glow::RED_INTEGER,
            glow::UNSIGNED_SHORT,
            glow::PixelUnpackData::Slice(None),
        );

        let depth = gl.create_texture()?;
        plain(depth);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::DEPTH_COMPONENT24 as i32,
            w,
            h,
            0,
            glow::DEPTH_COMPONENT,
            glow::UNSIGNED_INT,
            glow::PixelUnpackData::Slice(None),
        );
        gl.bind_texture(glow::TEXTURE_2D, None);

        let scene = gl.create_framebuffer()?;
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(scene));
        gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(colour), 0);
        gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT1, glow::TEXTURE_2D, Some(tags), 0);
        gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::DEPTH_ATTACHMENT, glow::TEXTURE_2D, Some(depth), 0);
        gl.draw_buffers(&[glow::COLOR_ATTACHMENT0, glow::COLOR_ATTACHMENT1]);
        let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
        if status != glow::FRAMEBUFFER_COMPLETE {
            return Err(format!("the offscreen target is not usable (status {status:#x})"));
        }

        // Colour only: the axis pass samples the depth and tag textures, and a
        // texture attached to the framebuffer being drawn into cannot be read.
        let overlay = gl.create_framebuffer()?;
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(overlay));
        gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(colour), 0);
        gl.draw_buffers(&[glow::COLOR_ATTACHMENT0]);
        let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
        if status != glow::FRAMEBUFFER_COMPLETE {
            return Err(format!("the overlay target is not usable (status {status:#x})"));
        }
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        Ok(Target { width, height, tags, depth, scene, overlay })
    }

    pub(super) unsafe fn destroy(&self, gl: &glow::Context) {
        gl.delete_framebuffer(self.scene);
        gl.delete_framebuffer(self.overlay);
        gl.delete_texture(self.tags);
        gl.delete_texture(self.depth);
        // The colour texture belongs to egui once it has been registered, so
        // it is deliberately not deleted here -- see `render`, which keeps one
        // texture for the life of the application and reallocates its storage.
    }
}
