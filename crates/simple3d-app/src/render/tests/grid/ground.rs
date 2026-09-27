//! The ground plane: what it covers, and what it must not draw over.

use super::*;
use crate::raster::{Image, Rgba};
use crate::view::View;
use simple3d_core::config::DisplayMode;
use simple3d_core::scene::AxisStyle;
use simple3d_core::scene::Camera;
use simple3d_geom::primitives;

#[test]
pub(crate) fn the_grid_never_draws_over_geometry_it_is_coplanar_with() {
    // A 4 mm plate's side walls are cut by the z=0 grid plane; the grid must not change any model pixel.
    let prepared = Renderable::prepare(&primitives::box_mesh(60.0, 40.0, 4.0));
    let build = |grid: bool| {
        let mut req = request(vec![Item { renderable: &prepared, style: Style::Solid }], DisplayMode::Shaded);
        req.grid = Grid { visible: grid, spacing: 10.0, axes: [true; 3], style: AxisStyle::Origin, plane_marks: false };
        req.view = req.view.with_camera(Camera { pitch: 10.0, ..req.view.camera() });
        // Axes are hidden so only the grid's effect on the coplanar model is tested.
        for axis in [&mut req.palette.axis_x, &mut req.palette.axis_y, &mut req.palette.axis_z] {
            axis[3] = 0;
        }
        (render(&req), req.palette)
    };
    let (without, palette) = build(false);
    let (with, _) = build(true);

    let mut checked = 0;
    for i in 0..(160 * 120) {
        let o = i * 4;
        let bare: Rgba = [without.color[o], without.color[o + 1], without.color[o + 2], without.color[o + 3]];
        if is_background(&without, i, &palette) {
            continue;
        }
        let gridded: Rgba = [with.color[o], with.color[o + 1], with.color[o + 2], with.color[o + 3]];
        assert_eq!(gridded, bare, "the grid overwrote the model at pixel {i}");
        checked += 1;
    }
    assert!(checked > 800, "only {checked} model pixels were checked");
}

/// A grid-only frame at `pitch`, so every non-background pixel is a grid line.
pub(crate) fn ground_only(pitch: f64) -> (Image, Palette) {
    let (w, h) = (400, 300);
    let camera = Camera { yaw: -55.0, pitch, distance: 900.0, ..Camera::default() };
    let view = View::new(camera, egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w as f32, h as f32)));
    let req = Request {
        view,
        size: [w, h],
        mode: DisplayMode::Shaded,
        palette: Palette::dark(),
        grid: Grid { visible: true, spacing: 10.0, axes: [false; 3], style: AxisStyle::Grid, plane_marks: false },
        items: Vec::new(),
        preview: Vec::new(),
        live: Live::default(),
        section: Vec::new(),
    };
    (render(&req), req.palette)
}

#[test]
pub(crate) fn the_ground_covers_the_viewport_at_every_tilt() {
    // Regression: the ground's extent assumed a top view, so tilted views stopped short (at 10 degrees
    // only rows 107 to 192 of 300). Measured along the middle row and column, not the faded corners.
    for pitch in [10.0, 28.0, 45.0, 60.0, 89.0] {
        let (frame, palette) = ground_only(pitch);
        let (w, h) = (frame.width, frame.height);
        let painted = |x: usize, y: usize| !is_background(&frame, y * w + x, &palette);
        let top = (0..h).find(|&y| painted(w / 2, y)).unwrap_or(h);
        let bottom = (0..h).rev().find(|&y| painted(w / 2, y)).unwrap_or(0);
        let left = (0..w).find(|&x| painted(x, h / 2)).unwrap_or(w);
        let right = (0..w).rev().find(|&x| painted(x, h / 2)).unwrap_or(0);
        // Within a seventh of each edge, since lines are a cell apart.
        assert!(top < h * 15 / 100, "pitch {pitch}: the ground starts {top} rows down, of {h}");
        assert!(bottom > h * 85 / 100, "pitch {pitch}: the ground ends at row {bottom}, of {h}");
        assert!(left < w * 15 / 100, "pitch {pitch}: the ground starts {left} columns in, of {w}");
        assert!(right > w * 85 / 100, "pitch {pitch}: the ground ends at column {right}, of {w}");
    }
}

#[test]
pub(crate) fn the_ground_is_drawn_at_one_detail_all_the_way_across() {
    // Regression: the fine level covered only half the coarse reach, making a detail patch around the
    // origin. A band at the edge must carry as much grid as one through the middle.
    for pitch in [10.0, 28.0, 45.0, 60.0, 89.0] {
        let (frame, palette) = ground_only(pitch);
        let (w, h) = (frame.width, frame.height);
        let band = |from: usize, to: usize| {
            (from..to).map(|y| (0..w).filter(|&x| !is_background(&frame, y * w + x, &palette)).count()).sum::<usize>()
        };
        let middle = band(h / 2 - 15, h / 2 + 15);
        let edge = band(h - 32, h - 2);
        assert!(middle > 0, "pitch {pitch}: nothing drawn across the middle, so this proves nothing");
        assert!(
            edge * 10 >= middle * 6,
            "pitch {pitch}: {edge} grid pixels at the edge against {middle} in the middle -- \
             the detail does not reach the edge of the frame"
        );
    }
}
