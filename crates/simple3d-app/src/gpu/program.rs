//! A compiled program and the buffers it draws from.

use super::*;
use eframe::glow::{self, HasContext};

pub(crate) struct Program {
    pub(super) program: glow::Program,
    pub(super) uniforms: std::collections::HashMap<String, glow::UniformLocation>,
}

pub(crate) struct Buffers {
    pub(super) array: glow::VertexArray,
    pub(super) vertices: glow::Buffer,
    /// The per-segment table of which bodies an axis may be seen through,
    /// uploaded as a texture because there are as many entries as there are
    /// bodies in the scene.
    pub(super) seen: glow::Texture,
}

impl Program {
    pub(super) unsafe fn new(gl: &glow::Context, vertex: &str, fragment: &str) -> Result<Program, String> {
        let program = gl.create_program()?;
        let mut shaders = Vec::new();
        for (kind, source) in [(glow::VERTEX_SHADER, vertex), (glow::FRAGMENT_SHADER, fragment)] {
            let shader = gl.create_shader(kind)?;
            gl.shader_source(shader, source);
            gl.compile_shader(shader);
            if !gl.get_shader_compile_status(shader) {
                let log = gl.get_shader_info_log(shader);
                gl.delete_shader(shader);
                for shader in shaders {
                    gl.delete_shader(shader);
                }
                gl.delete_program(program);
                return Err(format!("shader would not compile: {log}"));
            }
            gl.attach_shader(program, shader);
            shaders.push(shader);
        }
        gl.link_program(program);
        for shader in shaders {
            gl.detach_shader(program, shader);
            gl.delete_shader(shader);
        }
        if !gl.get_program_link_status(program) {
            let log = gl.get_program_info_log(program);
            gl.delete_program(program);
            return Err(format!("shaders would not link: {log}"));
        }
        let mut uniforms = std::collections::HashMap::new();
        let count = gl.get_active_uniforms(program);
        for index in 0..count {
            if let Some(uniform) = gl.get_active_uniform(program, index) {
                if let Some(location) = gl.get_uniform_location(program, &uniform.name) {
                    uniforms.insert(uniform.name, location);
                }
            }
        }
        Ok(Program { program, uniforms })
    }

    pub(super) fn at(&self, name: &str) -> Option<&glow::UniformLocation> {
        self.uniforms.get(name)
    }
}

impl Buffers {
    pub(super) unsafe fn new(gl: &glow::Context) -> Result<Buffers, String> {
        let array = gl.create_vertex_array()?;
        let vertices = gl.create_buffer()?;
        gl.bind_vertex_array(Some(array));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertices));
        let stride = std::mem::size_of::<GpuVertex>() as i32;
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 1, glow::FLOAT, false, stride, 8);
        // Normalised, so the shader sees the same 0..1 the palette's bytes mean.
        gl.enable_vertex_attrib_array(2);
        gl.vertex_attrib_pointer_f32(2, 4, glow::UNSIGNED_BYTE, true, stride, 12);
        gl.enable_vertex_attrib_array(3);
        gl.vertex_attrib_pointer_i32(3, 1, glow::UNSIGNED_INT, stride, 16);
        gl.enable_vertex_attrib_array(4);
        gl.vertex_attrib_pointer_i32(4, 1, glow::UNSIGNED_INT, stride, 20);
        gl.bind_vertex_array(None);

        let seen = gl.create_texture()?;
        gl.bind_texture(glow::TEXTURE_2D, Some(seen));
        for (name, value) in [
            (glow::TEXTURE_MIN_FILTER, glow::NEAREST),
            (glow::TEXTURE_MAG_FILTER, glow::NEAREST),
            (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
            (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
        ] {
            gl.tex_parameter_i32(glow::TEXTURE_2D, name, value as i32);
        }
        gl.bind_texture(glow::TEXTURE_2D, None);
        Ok(Buffers { array, vertices, seen })
    }
}
