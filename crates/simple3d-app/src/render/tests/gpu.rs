//! The GPU renderer on a real card, headless: an EGL context on the device itself, with no window
//! and no display, so it runs in a test and touches no one's screen.
//!
//! Skipped where there is no hardware GPU, as on CI. Software GL is skipped too: llvmpipe returns
//! zero screen derivatives after a `discard` and does not number primitives per triangle, both of
//! which the face shader relies on, so its pictures say nothing about real drivers.

use super::*;
use eframe::glow::{self, HasContext};
use glutin::api::egl::{context::PossiblyCurrentContext, device::Device, display::Display};
use glutin::config::{ConfigSurfaceTypes, ConfigTemplateBuilder};
use glutin::context::{ContextApi, ContextAttributesBuilder, GlProfile, Version};
use glutin::prelude::*;
use std::sync::Arc;

/// A current GL context on a hardware GPU, kept alive with what it was made from.
pub(super) struct Headless {
    pub gl: Arc<glow::Context>,
    _context: PossiblyCurrentContext,
    _display: Display,
}

/// The first hardware GPU's context, or `None` (with the reason printed) where there is none.
pub(super) fn headless() -> Option<Headless> {
    let devices = match Device::query_devices() {
        Ok(devices) => devices,
        Err(why) => {
            eprintln!("skipped: no EGL devices ({why})");
            return None;
        }
    };
    for device in devices {
        let Some(display) = (unsafe { Display::with_device(&device, None) }).ok() else { continue };
        let template = ConfigTemplateBuilder::new().with_surface_type(ConfigSurfaceTypes::empty()).build();
        let Some(config) = (unsafe { display.find_configs(template) }).ok().and_then(|mut c| c.next()) else {
            continue;
        };
        let attributes = ContextAttributesBuilder::new()
            .with_profile(GlProfile::Core)
            .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))))
            .build(None);
        let Some(context) = (unsafe { display.create_context(&config, &attributes) }).ok() else { continue };
        let Some(context) = context.make_current_surfaceless().ok() else { continue };
        let gl = unsafe { glow::Context::from_loader_function_cstr(|name| display.get_proc_address(name)) };
        let renderer = unsafe { gl.get_parameter_string(glow::RENDERER) };
        if ["llvmpipe", "softpipe", "swrast"].iter().any(|soft| renderer.contains(soft)) {
            eprintln!("skipped a software renderer: {renderer}");
            continue;
        }
        return Some(Headless { gl: Arc::new(gl), _context: context, _display: display });
    }
    eprintln!("skipped: no hardware GPU");
    None
}

/// `req` drawn by the GPU renderer, as RGBA rows from the top like the CPU's [`Image`].
pub(super) fn gpu_render(headless: &Headless, req: &Request<'_>) -> Vec<u8> {
    let gl = &headless.gl;
    let mut gpu = crate::gpu::Gpu::new(gl.clone()).expect("the shaders compile");
    let texture = gpu.prepare_texture().expect("a texture");
    gpu.set_texture_id(egui::TextureId::User(1));
    gpu.render(req).expect("the frame is drawn");
    let [width, height] = req.size;
    let mut pixels = vec![0u8; width * height * 4];
    unsafe {
        let fbo = gl.create_framebuffer().expect("a framebuffer");
        gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(fbo));
        gl.framebuffer_texture_2d(glow::READ_FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(texture), 0);
        let slice = glow::PixelPackData::Slice(Some(&mut pixels));
        gl.read_pixels(0, 0, width as i32, height as i32, glow::RGBA, glow::UNSIGNED_BYTE, slice);
        gl.bind_framebuffer(glow::READ_FRAMEBUFFER, None);
        gl.delete_framebuffer(fbo);
    }
    pixels
}

/// Smooth shading's GPU half: corner normals blended per pixel on the card, against the software
/// renderer's blended corner colours. Measured on an RTX 4090: 108 of 307200 pixels more than 3
/// levels apart; with the GPU's smoothing switched off, so mesh bodies drew flat, 6765.
#[test]
fn smooth_shading_on_the_gpu_matches_the_software_renderer() {
    let Some(headless) = headless() else { return };
    let with = |mut mesh: simple3d_geom::Mesh, source: u32, x: f64, y: f64| {
        mesh.set_source(source);
        mesh.translated(simple3d_geom::Vec3::new(x, y, 0.0))
    };
    // A flat sphere beside smooth ones, a smooth cylinder and box, and a painted smooth sphere.
    let mut scene = with(primitives::ellipsoid_mesh(30.0, 30.0, 30.0, 24), 1, -40.0, 0.0);
    scene.append(&with(primitives::ellipsoid_mesh(30.0, 30.0, 30.0, 24), 7, 0.0, 0.0));
    scene.append(&with(primitives::cylinder_mesh(25.0, 25.0, 30.0, 24), 7, 40.0, 0.0));
    scene.append(&with(primitives::box_mesh(25.0, 25.0, 25.0), 7, -40.0, 40.0));
    let mut painted = primitives::ellipsoid_mesh(30.0, 30.0, 30.0, 24);
    painted.set_tag(simple3d_geom::colour_tag([0xE0, 0x70, 0x30]));
    scene.append(&with(painted, 7, 10.0, 40.0));
    let prepared = Renderable::prepare_scene(&scene, &Default::default(), &[7].into());
    let mut req = request(vec![Item { renderable: &prepared, style: Style::Solid }], DisplayMode::Shaded);
    req.size = [640, 480];
    req.view = View::new(
        simple3d_core::scene::Camera { yaw: -55.0, pitch: 28.0, distance: 220.0, ..Default::default() },
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(640.0, 480.0)),
    );
    req.grid.axes = [false; 3];
    let cpu = render(&req).color;
    let gpu = gpu_render(&headless, &req);
    // More than 3 levels apart: the two blend differently (per corner on the CPU, per pixel here),
    // which shows only along silhouettes.
    let apart =
        cpu.chunks_exact(4).zip(gpu.chunks_exact(4)).filter(|(c, g)| (0..3).any(|k| c[k].abs_diff(g[k]) > 3)).count();
    assert!(apart < 1000, "{apart} pixels differ between the renderers");
}
