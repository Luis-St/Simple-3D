//! What is drawn over the picture while sketching: the picked regions, the curves and points coloured
//! by how far the constraints hold them, the gesture in progress, and the constraint labels.

use super::*;
use crate::app::App;
use crate::theme::token;
use crate::view::View;
use simple3d_sketch::sample_curve;

/// Copied model edges, apart from what is drawn.
const REFERENCE: egui::Color32 = egui::Color32::from_rgb(0xB0, 0x8C, 0xE0);
/// Pieces a full circle is drawn with.
const DRAWN_SEGMENTS: u32 = 96;

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
    draw_axes(tool, painter, view);
    draw_regions(tool, painter, view, hover);
    for curve in &tool.sketch.curves {
        draw_curve(tool, painter, view, curve);
    }
    for point in &tool.sketch.points {
        let Some(at) = tool.screen(view, point.at) else { continue };
        let pick = Pick::Point(point.id);
        let colour = if tool.selection.contains(&pick) || tool.hover == Some(pick) {
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
    draw_gesture(app, painter, view, hover);
    let Some(tool) = app.sketch_tool.as_ref() else { return };
    for label in super::glyphs::labels(tool, view) {
        let pick = Pick::Constraint(label.id);
        let colour = if tool.conflicts.contains(&label.id) || tool.analysis.redundant.contains(&label.id) {
            token::DANGER
        } else if tool.selection.contains(&pick) || tool.hover == Some(pick) {
            token::ACCENT
        } else if label.dimension {
            token::MEASURE
        } else {
            token::TEXT_LO
        };
        let rect = label.rect();
        painter.rect_filled(rect, 3.0, token::SURFACE_1.gamma_multiply(0.85));
        painter.text(rect.center(), egui::Align2::CENTER_CENTER, &label.text, egui::FontId::monospace(11.0), colour);
    }
}

/// The sketch's origin and its X and Y, so the plane reads as a plane before anything is drawn.
fn draw_axes(tool: &SketchTool, painter: &egui::Painter, view: &View) {
    let Some(origin) = tool.screen(view, [0.0, 0.0]) else { return };
    let arm = 40.0 / view.pixels_per_mm().max(1e-9);
    for (axis, end) in [(0, [arm, 0.0]), (1, [0.0, arm])] {
        if let Some(end) = tool.screen(view, end) {
            painter.line_segment(
                [origin, end],
                egui::Stroke::new(1.5_f32, crate::gizmo::axis_colour(axis).gamma_multiply(0.7)),
            );
        }
    }
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

fn draw_curve(tool: &SketchTool, painter: &egui::Painter, view: &View, curve: &simple3d_sketch::Curve) {
    let samples = sample_curve(&tool.sketch, curve, DRAWN_SEGMENTS);
    let mut points: Vec<egui::Pos2> = samples.points.iter().filter_map(|&p| tool.screen(view, p)).collect();
    if samples.closed && !points.is_empty() {
        points.push(points[0]);
    }
    let pick = Pick::Curve(curve.id);
    let colour = if tool.selection.contains(&pick) || tool.hover == Some(pick) {
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

/// The curve the gesture in progress would add, from its clicks to the pointer.
fn draw_gesture(app: &App, painter: &egui::Painter, view: &View, hover: Option<egui::Pos2>) {
    let Some(tool) = app.sketch_tool.as_ref() else { return };
    if tool.clicks.is_empty() {
        return;
    }
    let spacing = if app.snap_requested { app.ground_grid(view).map(|grid| grid.spacing) } else { None };
    let Some(next) = hover.and_then(|cursor| tool.click_at(view, cursor, spacing)) else { return };
    let mut sketch = Sketch::default();
    let ids: Vec<PointId> = tool.clicks.iter().chain(std::iter::once(&next)).map(|c| sketch.add_point(c.at)).collect();
    use simple3d_sketch::Shape;
    let shapes: Vec<Shape> = match tool.draw {
        Draw::Line => vec![Shape::Line { a: ids[ids.len() - 2], b: ids[ids.len() - 1] }],
        Draw::Rectangle => {
            let (a, b) = (tool.clicks[0].at, next.at);
            let corners = [a, [b[0], a[1]], b, [a[0], b[1]]].map(|at| sketch.add_point(at));
            (0..4).map(|i| Shape::Line { a: corners[i], b: corners[(i + 1) % 4] }).collect()
        }
        Draw::Circle => {
            let (c, p) = (tool.clicks[0].at, next.at);
            vec![Shape::Circle { center: ids[0], radius: (p[0] - c[0]).hypot(p[1] - c[1]) }]
        }
        Draw::Arc if ids.len() == 3 => vec![Shape::Arc { center: ids[0], start: ids[1], end: ids[2] }],
        Draw::Arc => vec![Shape::Line { a: ids[0], b: ids[1] }],
        Draw::Spline => vec![Shape::Spline { points: ids.clone(), closed: false }],
        Draw::Bspline => vec![Shape::Bspline { points: ids.clone(), closed: false }],
        _ => Vec::new(),
    };
    let stroke = egui::Stroke::new(1.5_f32, token::ACCENT.gamma_multiply(0.8));
    for shape in shapes {
        let curve = simple3d_sketch::Curve { id: 0, shape, construction: false, reference: false };
        let line: Vec<egui::Pos2> =
            sample_curve(&sketch, &curve, DRAWN_SEGMENTS).points.iter().filter_map(|&p| tool.screen(view, p)).collect();
        painter.extend(egui::Shape::dashed_line(&line, stroke, 6.0, 3.0));
    }
}
