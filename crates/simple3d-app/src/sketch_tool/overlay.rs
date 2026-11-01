//! What is drawn over the picture while sketching: the picked regions, the curves and points coloured
//! by how far the constraints hold them, every line's length, the gesture in progress, the
//! constraint marks and dimensions, and what a click would infer or place.

use super::*;
use crate::app::App;
use crate::snap::SnapMark;
use crate::theme::token;
use crate::view::View;
use simple3d_sketch::sample_curve;

/// Copied model edges, apart from what is drawn.
const REFERENCE: egui::Color32 = egui::Color32::from_rgb(0xB0, 0x8C, 0xE0);
/// Pieces a full circle is drawn with.
pub(crate) const DRAWN_SEGMENTS: u32 = 96;

pub(crate) fn draw(app: &mut App, ui: &egui::Ui, painter: &egui::Painter, view: &View) {
    let unit = app.unit();
    let hover = ui.input(|i| i.pointer.hover_pos()).filter(|at| painter.clip_rect().contains(*at));
    let Some(tool) = app.sketch_tool.as_ref() else { return };
    if tool.frame.is_none() {
        // The origin planes, and the face a click would put the sketch on when it is on none of them.
        let plane = hover.and_then(|cursor| super::origin::plane_at(view, cursor));
        super::origin::draw(painter, view, plane);
        let face = hover.filter(|_| plane.is_none()).and_then(|cursor| crate::cut_tool::face_under(app, view, cursor));
        if let Some(face) = face {
            let (u, v) = face.axes();
            let size = 14.0 / view.pixels_per_mm().max(1e-9);
            let corner = |s: f64, t: f64| view.project(face.point + u * (s * size) + v * (t * size)).map(|p| p.0);
            let corners: Vec<egui::Pos2> = [corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)]
                .into_iter()
                .flatten()
                .collect();
            painter
                .add(egui::Shape::closed_line(corners, egui::Stroke::new(1.5_f32, token::ACCENT.gamma_multiply(0.6))));
        }
        return;
    }
    if let Some(tool) = app.sketch_tool.as_mut() {
        tool.unit = unit;
    }
    let Some(tool) = app.sketch_tool.as_ref() else { return };
    draw_axes(tool, painter, view, app.shown_axes());
    draw_regions(tool, painter, view, hover);
    let (lit_curves, lit_points) = super::glyphs::lit(tool);
    for curve in &tool.sketch.curves {
        draw_curve(tool, painter, view, curve, lit_curves.contains(&curve.id));
    }
    for point in &tool.sketch.points {
        let Some(at) = tool.screen(view, point.at) else { continue };
        let pick = Pick::Point(point.id);
        let colour = if tool.selection.contains(&pick) || tool.hover == Some(pick) || lit_points.contains(&point.id) {
            token::ACCENT
        } else if point.fixed {
            REFERENCE
        } else if tool.analysis.free_points.contains(&point.id) {
            token::MEASURE
        } else {
            token::TEXT_HI
        };
        painter.rect_filled(egui::Rect::from_center_size(at, egui::Vec2::splat(5.0)), 1.0, colour);
    }
    super::readout::lengths(tool, painter, view);
    super::gesture::draw(app, painter, view, hover);
    if let Some(tool) = app.sketch_tool.as_ref() {
        super::area::draw(tool, painter);
    }
    // Where a click or drag lands, marked as every other snap is.
    if let Some(at) = hover.and_then(|cursor| snap_landing(app, view, cursor)) {
        if let Some(tool) = app.sketch_tool.as_ref() {
            crate::panel_viewport::draw_snap_mark(painter, view, &SnapMark::at(tool.world(at)), token::ACCENT);
        }
    }
    let Some(tool) = app.sketch_tool.as_ref() else { return };
    super::glyphs::draw(tool, painter, view);
    if let Some(cursor) = hover {
        super::infer_draw::draw(app, painter, view, cursor);
    }
}

/// Where a click or drag at `cursor` snaps to, in sketch coordinates: a sketch point caught by a
/// drawing click, or with the snap key held the grid crossing a click or the dragged point lands on.
/// `None` when nothing snaps.
pub(crate) fn snap_landing(app: &App, view: &View, cursor: egui::Pos2) -> Option<[f64; 2]> {
    let tool = app.sketch_tool.as_ref()?;
    let step = super::grid_step(app, view);
    if let Some(drag) = &tool.drag {
        let (step, from, at) = (step?, drag.anchor()?, tool.under(view, cursor)?);
        let delta = super::pointer::snapped_delta(drag, [at[0] - drag.start[0], at[1] - drag.start[1]], Some(step));
        return Some([from[0] + delta[0], from[1] + delta[1]]);
    }
    if !tool.draw.draws() && tool.draw.selection_clicks() == 0 {
        return None;
    }
    let (click, inferred) = tool.drawing_click(view, cursor, step)?;
    (click.point.is_some() || step.is_some() || !inferred.is_empty()).then_some(click.at)
}

/// The sketch's origin and its X and Y, so the plane reads as a plane before anything is drawn. An
/// arm a shown world axis already runs along is left out: drawn over it a pixel off, it made the
/// axis look twice as thick near the origin.
fn draw_axes(tool: &SketchTool, painter: &egui::Painter, view: &View, shown: [bool; 3]) {
    let Some(origin) = tool.screen(view, [0.0, 0.0]) else { return };
    let arm = 40.0 / view.pixels_per_mm().max(1e-9);
    for (axis, end) in [(0, [arm, 0.0]), (1, [0.0, arm])] {
        if on_world_axis(tool, axis, shown) {
            continue;
        }
        if let Some(end) = tool.screen(view, end) {
            painter.line_segment(
                [origin, end],
                egui::Stroke::new(1.5_f32, crate::gizmo::axis_colour(axis).gamma_multiply(0.7)),
            );
        }
    }
}

/// Whether sketch axis `axis` (0 for X, 1 for Y) lies along a world axis that is drawn.
pub(crate) fn on_world_axis(tool: &SketchTool, axis: usize, shown: [bool; 3]) -> bool {
    let Some(frame) = tool.frame else { return false };
    let (origin, direction) = (frame.t, frame.axis(axis));
    (0..3).any(|world| {
        let along = direction.get(world).abs() > 1.0 - 1e-6;
        let through = (0..3).filter(|&k| k != world).all(|k| origin.get(k).abs() < 1e-6);
        shown[world] && along && through
    })
}

/// The picked regions filled, and in the region tool the one under the pointer faintly.
fn draw_regions(tool: &SketchTool, painter: &egui::Painter, view: &View, hover: Option<egui::Pos2>) {
    let regions = simple3d_sketch::regions::find(&tool.sketch, DRAWN_SEGMENTS);
    let picked = regions.resolve_all(&tool.sketch.regions);
    let hovered = match (tool.draw, hover) {
        (Draw::Regions, Some(cursor)) => tool.under(view, cursor).and_then(|at| regions.at(at)),
        _ => None,
    };
    let fill = |indices: &[usize], colour: egui::Color32| {
        for outline in regions.outlines(indices) {
            fill_outline(tool, painter, view, &outline, colour);
        }
    };
    fill(&picked, token::ACCENT.gamma_multiply(0.22));
    if let Some(index) = hovered.filter(|index| !picked.contains(index)) {
        fill(&[index], token::ACCENT.gamma_multiply(0.10));
    }
}

/// An outline filled on screen, holes left open, by triangulating it.
fn fill_outline(
    tool: &SketchTool,
    painter: &egui::Painter,
    view: &View,
    outline: &simple3d_geom::push_pull::Outline,
    colour: egui::Color32,
) {
    let mut positions: Vec<Vec3> = Vec::new();
    let mut loops: Vec<Vec<u32>> = Vec::new();
    for points in std::iter::once(&outline.outer).chain(outline.holes.iter()) {
        let start = positions.len() as u32;
        loops.push((0..points.len() as u32).map(|k| start + k).collect());
        positions.extend(points.iter().map(|p| Vec3::new(p[0], p[1], 0.0)));
    }
    let Some(triangles) = simple3d_geom::planar::triangulate_loops(&positions, Vec3::new(0.0, 0.0, 1.0), loops) else {
        return;
    };
    let mut mesh = egui::Mesh::default();
    for p in &positions {
        let at = tool.screen(view, [p.x, p.y]).unwrap_or(egui::Pos2::ZERO);
        mesh.colored_vertex(at, colour);
    }
    for [a, b, c] in triangles {
        mesh.add_triangle(a, b, c);
    }
    painter.add(egui::Shape::mesh(mesh));
}

fn draw_curve(tool: &SketchTool, painter: &egui::Painter, view: &View, curve: &simple3d_sketch::Curve, lit: bool) {
    let samples = sample_curve(&tool.sketch, curve, DRAWN_SEGMENTS);
    let mut points: Vec<egui::Pos2> = samples.points.iter().filter_map(|&p| tool.screen(view, p)).collect();
    if samples.closed && !points.is_empty() {
        points.push(points[0]);
    }
    let pick = Pick::Curve(curve.id);
    let colour = if tool.selection.contains(&pick) || tool.hover == Some(pick) || lit {
        token::ACCENT
    } else if curve.reference {
        REFERENCE
    } else if tool.analysis.free_curves.contains(&curve.id) {
        token::MEASURE
    } else {
        token::TEXT_HI
    };
    let axis = tool.sketch.axis == Some(curve.id);
    let width = if axis { 2.5_f32 } else { 1.8 };
    let stroke = egui::Stroke::new(width, colour);
    if curve.construction || axis {
        painter.extend(egui::Shape::dashed_line(&points, stroke, 7.0, 4.0));
    } else {
        painter.add(egui::Shape::line(points, stroke));
    }
}
