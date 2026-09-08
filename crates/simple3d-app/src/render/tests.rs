mod axes;
mod axes_occlusion;
mod bands;
mod grid;
mod preview;
mod section;
mod selection_creases;
mod selection_outline;
mod shading;

use super::*;
use crate::raster::{Image, Rgba};
use crate::view::View;
use simple3d_core::config::DisplayMode;
use simple3d_core::scene::AxisStyle;
// The tests exercise these modules' own workings, not only what the
// renderer re-exports.
use super::frame::*;
use super::renderable::*;
use simple3d_core::scene::Camera;
use simple3d_geom::primitives;

fn view(width: usize, height: usize) -> View {
    View::new(
        Camera { yaw: -55.0, pitch: 28.0, distance: 120.0, ..Camera::default() },
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width as f32, height as f32)),
    )
}

fn request<'a>(items: Vec<Item<'a>>, mode: DisplayMode) -> Request<'a> {
    Request {
        view: view(160, 120),
        size: [160, 120],
        mode,
        palette: Palette::dark(),
        grid: Grid { visible: false, spacing: 10.0, axes: [true; 3], style: AxisStyle::Origin, plane_marks: false },
        items,
        preview: Vec::new(),
        section: None,
    }
}

fn count_non_background(frame: &Image, palette: &Palette) -> usize {
    (0..frame.height * frame.width).filter(|&i| !is_background(frame, i, palette)).count()
}

/// Whether pixel `index` still holds the gradient it was cleared to.
fn is_background(frame: &Image, index: usize, palette: &Palette) -> bool {
    let offset = index * 4;
    let pixel: Rgba = [frame.color[offset], frame.color[offset + 1], frame.color[offset + 2], frame.color[offset + 3]];
    pixel == palette.background_at(index / frame.width, frame.height)
}

/// How many pixels of the frame carry the given colour, shaded or not.
/// A drawn line keeps its colour exactly; only shaded faces are scaled.
fn pixels_of(frame: &Image, colour: Rgba) -> usize {
    (0..frame.width * frame.height)
        .filter(|&i| {
            let o = i * 4;
            frame.color[o] == colour[0] && frame.color[o + 1] == colour[1] && frame.color[o + 2] == colour[2]
        })
        .count()
}

fn shifted_box(x: f64) -> simple3d_geom::Mesh {
    let mut mesh = primitives::box_mesh(20.0, 20.0, 20.0);
    for p in &mut mesh.positions {
        p.x += x;
    }
    mesh
}

/// The scene from straight in front, where every side face is edge-on.
fn straight_on(items: Vec<Item<'_>>) -> Image {
    let mut req = request(items, DisplayMode::Shaded);
    req.view = View::new(
        Camera { yaw: -90.0, pitch: 0.0, distance: 90.0, ..Camera::default() },
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(160.0, 120.0)),
    );
    render(&req)
}

fn column_has(frame: &Image, x: usize, colour: Rgba) -> bool {
    rows_of(frame, x, colour, 0..frame.height)
}

/// Whether a column carries the colour anywhere in the *middle* of the
/// shape, away from the rows the top and bottom of an outline run along --
/// a vertical line is only a vertical line if it is there between them.
fn column_has_between(frame: &Image, x: usize, colour: Rgba, from: usize, to: usize) -> bool {
    rows_of(frame, x, colour, from..to)
}

fn rows_of(frame: &Image, x: usize, colour: Rgba, rows: std::ops::Range<usize>) -> bool {
    rows.into_iter().any(|y| {
        let o = (y * frame.width + x) * 4;
        frame.color[o] == colour[0] && frame.color[o + 1] == colour[1] && frame.color[o + 2] == colour[2]
    })
}

/// How many separate stretches of `colour` a walk over `path` crosses. A
/// count of the lines met, rather than of the pixels they cover.
fn runs(frame: &Image, colour: Rgba, path: impl Iterator<Item = (usize, usize)>) -> usize {
    let (mut count, mut on) = (0, false);
    for (x, y) in path {
        let here = rows_of(frame, x, colour, y..y + 1);
        if here && !on {
            count += 1;
        }
        on = here;
    }
    count
}

/// The first and last column carrying the outline's colour.
fn accent_span(frame: &Image, colour: Rgba) -> (usize, usize) {
    let columns: Vec<usize> = (0..frame.width).filter(|&x| column_has(frame, x, colour)).collect();
    (*columns.first().expect("nothing was outlined at all"), *columns.last().unwrap())
}

/// The first and last row carrying it, so a test can keep away from the
/// horizontal parts of the outline.
fn accent_rows(frame: &Image, colour: Rgba) -> (usize, usize) {
    let rows: Vec<usize> =
        (0..frame.height).filter(|&y| (0..frame.width).any(|x| rows_of(frame, x, colour, y..y + 1))).collect();
    (*rows.first().expect("nothing was outlined at all"), *rows.last().unwrap())
}
