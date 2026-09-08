mod cube;
mod frame;
mod project;

use super::cube::*;
use super::frame::*;
use super::preset::*;
use super::*;
use simple3d_core::scene::Camera;
use simple3d_geom::Vec3;

fn view() -> View {
    let camera = Camera { yaw: -90.0, pitch: 0.0, distance: 100.0, ..Camera::default() };
    View::new(camera, egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0)))
}

fn close(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < 1e-6
}
