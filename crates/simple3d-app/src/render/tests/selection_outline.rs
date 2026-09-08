//! The outline round a selected body.

mod coverage;
mod sides;

use super::*;
use crate::raster::{Image, Rgba};
use simple3d_core::config::DisplayMode;

/// The convex hull of a set of screen points, counter-clockwise in
/// screen coordinates. Andrew's monotone chain.
pub(crate) fn hull(mut points: Vec<egui::Pos2>) -> Vec<egui::Pos2> {
    points.sort_by(|a, b| (a.x, a.y).partial_cmp(&(b.x, b.y)).unwrap());
    points.dedup();
    let cross = |o: egui::Pos2, a: egui::Pos2, b: egui::Pos2| (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
    let mut out: Vec<egui::Pos2> = Vec::new();
    for pass in 0..2 {
        let start = (out.len() + 1).max(2);
        let iter: Box<dyn Iterator<Item = &egui::Pos2>> =
            if pass == 0 { Box::new(points.iter()) } else { Box::new(points.iter().rev()) };
        for &p in iter {
            while out.len() >= start && cross(out[out.len() - 2], out[out.len() - 1], p) <= 0.0 {
                out.pop();
            }
            out.push(p);
        }
        out.pop();
    }
    out
}

/// Pixels that lie at least `margin` inside the hull but were never painted.
/// For a convex solid the painted silhouette *is* the hull of its projected
/// vertices, so any such pixel means a face that faces the viewer was not
/// drawn.
pub(crate) fn unpainted_inside(hull: &[egui::Pos2], frame: &Image, empty: &Image, margin: f32) -> usize {
    let mut missing = 0;
    for row in 0..frame.height {
        for x in 0..frame.width {
            let p = egui::pos2(x as f32 + 0.5, row as f32 + 0.5);
            let inside = hull.windows(2).chain(std::iter::once([hull[hull.len() - 1], hull[0]].as_slice())).all(|e| {
                let (a, b) = (e[0], e[1]);
                let n = egui::vec2(b.y - a.y, a.x - b.x);
                let len = n.length().max(1e-6);
                ((p - a).dot(n) / len) <= -margin
            });
            let o = (row * frame.width + x) * 4;
            if inside && frame.color[o..o + 4] == empty.color[o..o + 4] {
                missing += 1;
            }
        }
    }
    missing
}

/// Where the selection colour was painted when `item` is drawn selected
/// over itself: how many pixels in all, and how many in the middle of the
/// frame.
///
/// The middle is the discriminator. An outline touches the shape's rim and
/// leaves the inside alone; a set of creases scribbles across it.
pub(crate) fn selection_coverage_of(item: &Renderable) -> (usize, usize) {
    let req = request(
        vec![Item { renderable: item, style: Style::Solid }, Item { renderable: item, style: Style::Selected }],
        DisplayMode::Shaded,
    );
    let frame = render(&req);
    let mut drawn = 0;
    let mut middle = 0;
    for y in 0..frame.height {
        for x in 0..frame.width {
            let offset = (y * frame.width + x) * 4;
            let pixel: Rgba =
                [frame.color[offset], frame.color[offset + 1], frame.color[offset + 2], frame.color[offset + 3]];
            if pixel != req.palette.selected {
                continue;
            }
            drawn += 1;
            let middling = (x as f32 - frame.width as f32 / 2.0).abs() < frame.width as f32 / 8.0
                && (y as f32 - frame.height as f32 / 2.0).abs() < frame.height as f32 / 8.0;
            if middling {
                middle += 1;
            }
        }
    }
    (drawn, middle)
}
