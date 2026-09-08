//! The cube's faces, sorted and drawn.

use crate::view::View;
use simple3d_geom::Vec3;

/// Put the corners of a face in ring order around its centre, so the quad drawn
/// from them is the face and not a bow tie.
pub(crate) fn sort_ring(mut points: Vec<egui::Pos2>, centre: egui::Pos2) -> Vec<egui::Pos2> {
    points.sort_by(|a, b| {
        let angle = |p: &egui::Pos2| (p.y - centre.y).atan2(p.x - centre.x);
        angle(a).partial_cmp(&angle(b)).unwrap_or(std::cmp::Ordering::Equal)
    });
    points
}

pub(crate) fn draw_box(painter: &egui::Painter, view: &View, lo: Vec3, hi: Vec3, colour: egui::Color32, width: f32) {
    let corner = |i: usize| {
        Vec3::new(
            if i & 1 == 0 { lo.x } else { hi.x },
            if i & 2 == 0 { lo.y } else { hi.y },
            if i & 4 == 0 { lo.z } else { hi.z },
        )
    };
    const EDGES: [(usize, usize); 12] =
        [(0, 1), (1, 3), (3, 2), (2, 0), (4, 5), (5, 7), (7, 6), (6, 4), (0, 4), (1, 5), (2, 6), (3, 7)];
    for (a, b) in EDGES {
        let (Some((pa, _)), Some((pb, _))) = (view.project(corner(a)), view.project(corner(b))) else {
            continue;
        };
        painter.line_segment([pa, pb], egui::Stroke::new(width, colour));
    }
}

/// The bounding box's dimensions, numerically -- the fastest way to answer "will
/// this fit" (spec section 6.1).
pub(crate) fn label_box(
    painter: &egui::Painter,
    ui: &egui::Ui,
    view: &View,
    lo: Vec3,
    hi: Vec3,
    unit: simple3d_core::unit::Unit,
    colour: egui::Color32,
) {
    let size = hi - lo;
    let centre = (lo + hi) * 0.5;
    let labels = [
        (Vec3::new(centre.x, lo.y, lo.z), simple3d_core::unit::format_length(size.x, unit)),
        (Vec3::new(hi.x, centre.y, lo.z), simple3d_core::unit::format_length(size.y, unit)),
        (Vec3::new(hi.x, lo.y, centre.z), simple3d_core::unit::format_length(size.z, unit)),
    ];
    for (world, text) in labels {
        let Some((screen, _)) = view.project(world) else { continue };
        let galley = painter.layout_no_wrap(format!("{text}{}", unit.suffix()), egui::FontId::monospace(11.0), colour);
        let at = screen + egui::vec2(6.0, -6.0);
        painter.rect_filled(
            egui::Rect::from_min_size(at, galley.size()).expand(2.0),
            2.0,
            ui.visuals().extreme_bg_color.gamma_multiply(0.75),
        );
        painter.galley(at, galley, colour);
    }
}
