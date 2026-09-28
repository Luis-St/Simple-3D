//! The outline round a selected body.

mod coverage;
mod sides;

use super::*;
use crate::raster::{Image, Rgba};
use simple3d_core::config::DisplayMode;

/// Convex hull of screen points, counter-clockwise (Andrew's monotone chain).
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

/// Unpainted pixels at least `margin` inside the hull; for a convex solid any means a missing front face.
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

/// Selection-colour pixels when `item` is drawn selected: total, and in the frame's middle. An
/// outline leaves the middle alone; creases scribble across it.
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
            // An ellipse, not the box around it: the box's corners reach a 40 mm sphere's rim, which
            // a two-pixel outline (issue 99) then touches without crossing the body.
            let dx = (x as f32 - frame.width as f32 / 2.0) / (frame.width as f32 / 8.0);
            let dy = (y as f32 - frame.height as f32 / 2.0) / (frame.height as f32 / 8.0);
            let middling = dx * dx + dy * dy < 1.0;
            if middling {
                middle += 1;
            }
        }
    }
    (drawn, middle)
}
