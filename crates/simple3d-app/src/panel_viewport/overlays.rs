//! What is drawn over the rendered image.

use super::*;
use crate::app::App;
use crate::gizmo::{self, Gizmo, Handle};
use crate::theme::{self, token};
use crate::view::View;

/// Everything over the rasterised image: manipulator, bounding boxes with dimensions, axis legend
/// and the live drag value.
pub(crate) fn overlays(app: &mut App, ui: &mut egui::Ui, rect: egui::Rect, view: &View) {
    let painter = ui.painter_at(rect);

    if app.settings.show_bounding_box {
        if let Some((lo, hi)) = app.selection_bounds() {
            draw_box(&painter, view, lo, hi, token::ACCENT, 1.5);
            // Dimensions are measurements, so they use the measure colour.
            label_box(&painter, ui, view, lo, hi, app.unit(), token::MEASURE);
        }
        if let Some((lo, hi)) = app.evaluated.bounds {
            draw_box(&painter, view, lo, hi, token::TEXT_LO.gamma_multiply(0.5), 1.0);
        }
    }

    if let Some(id) = app.primary() {
        if let Some(gizmo) = app.gizmo_for(id) {
            draw_gizmo(app, &painter, ui, &gizmo, view, app.scene.node(id).is_group() || app.scene.node(id).is_split());
        }
    }

    // The 3D cursor, where the next shape would land.
    draw_cursor(app, &painter, view);

    // A snapped feature (issue 68), with the edges and corners of what it caught (issue 87).
    if let Some(mark) = &app.snap_indicator {
        draw_snap_mark(&painter, view, mark, token::ACCENT);
    }

    // Pattern lay-out grips (issue 67).
    draw_pattern_grips(app, &painter, view);

    // Rule placements while the tool is open on an empty pattern (issue 96).
    draw_pattern_placements(app, &painter, view);

    // The section frame and grip (issue 71), over the image since nothing kept is in front of it.
    crate::section_tool::draw(app, &painter, view);

    if app.measure.active {
        draw_measure(app, ui, &painter, view);
    }

    // Only the held tool is shown; projection and handle frame were dropped as they never change
    // (issue 100). The measure tool is named while open.
    let tool = if app.measure.active { "Measure" } else { app.mode.label() };
    let galley =
        painter.layout_no_wrap(tool.to_string(), egui::FontId::proportional(theme::font::SMALL), token::TEXT_LO);
    let at = rect.left_top() + egui::vec2(10.0, 8.0);
    painter.rect_filled(
        egui::Rect::from_min_size(at, galley.size()).expand2(egui::vec2(6.0, 3.0)),
        3.0,
        token::SURFACE_1.gamma_multiply(0.72),
    );
    painter.galley(at, galley, token::TEXT_LO);

    // The live numeric value at the cursor during a drag.
    if let Some(drag) = &app.drag {
        if let Some(cursor) = ui.input(|i| i.pointer.hover_pos()) {
            let at = cursor + egui::vec2(14.0, -18.0);
            // Cyan monospace: a measurement, not a message.
            let galley = painter.layout_no_wrap(drag.readout.clone(), egui::FontId::monospace(13.0), token::MEASURE);
            let background = egui::Rect::from_min_size(at, galley.size()).expand(5.0);
            painter.rect_filled(background, 3.0, token::SURFACE_1.gamma_multiply(0.92));
            painter.rect_stroke(
                background,
                3.0,
                egui::Stroke::new(1.0_f32, token::MEASURE.gamma_multiply(0.5)),
                egui::StrokeKind::Inside,
            );
            painter.galley(at, galley, token::MEASURE);
        }
    }
}

pub(crate) fn draw_gizmo(
    app: &App,
    painter: &egui::Painter,
    ui: &egui::Ui,
    gizmo: &Gizmo,
    view: &View,
    is_group: bool,
) {
    let handles = gizmo.handles(is_group);
    if handles.is_empty() {
        if app.mode == gizmo::Mode::Resize {
            // Say why resizing is unavailable.
            let Some((origin, _)) = view.project(gizmo.origin) else { return };
            // On its own plate, since plain weak text vanished over grey faces.
            let galley = painter.layout_no_wrap(
                if is_group { "Groups have no resize handles" } else { "This shape has no resizable axis" }.to_string(),
                egui::FontId::proportional(11.0),
                ui.visuals().weak_text_color(),
            );
            let at = origin + egui::vec2(12.0, 12.0);
            painter.rect_filled(
                egui::Rect::from_min_size(at, galley.size()).expand2(egui::vec2(6.0, 3.0)),
                3.0,
                token::SURFACE_1.gamma_multiply(0.85),
            );
            painter.galley(at, galley, ui.visuals().weak_text_color());
        }
        return;
    }
    let Some((origin, _)) = view.project(gizmo.origin) else { return };
    let active = app.drag.as_ref().map(|d| d.handle);
    let highlight = |handle: Handle| active == Some(handle) || (active.is_none() && app.hover_handle == Some(handle));

    for handle in handles {
        let axes = handle.axes();
        let colour = if axes.len() == 1 { gizmo::axis_colour(axes[0]) } else { egui::Color32::from_rgb(200, 200, 210) };
        let colour = if highlight(handle) { egui::Color32::from_rgb(255, 214, 96) } else { colour };
        match handle {
            Handle::MoveAxis(_) => {
                let Some((tip, _)) = view.project(gizmo.handle_point(handle, view)) else { continue };
                painter.line_segment([origin, tip], egui::Stroke::new(2.0_f32, colour));
                painter.circle_filled(tip, if highlight(handle) { 6.0 } else { 4.5 }, colour);
            }
            Handle::MovePlane(axis) => {
                // Not drawn edge-on, see `Gizmo::plane_quad`.
                let Some(quad) = gizmo.plane_quad(axis, view) else { continue };
                let fill = gizmo::axis_colour(axis).gamma_multiply(if highlight(handle) { 0.55 } else { 0.25 });
                painter.add(egui::Shape::convex_polygon(quad.to_vec(), fill, egui::Stroke::new(1.0_f32, colour)));
            }
            Handle::RotateRing(axis) => {
                let points: Vec<egui::Pos2> = gizmo
                    .ring_points(axis, view, 64)
                    .into_iter()
                    .filter_map(|p| view.project(p).map(|(screen, _)| screen))
                    .collect();
                if points.len() > 2 {
                    painter.add(egui::Shape::closed_line(points, egui::Stroke::new(2.0_f32, colour)));
                }
            }
            Handle::ResizeFace(_, _) => {
                let Some((at, _)) = view.project(gizmo.handle_point(handle, view)) else { continue };
                let size = if highlight(handle) { 5.5 } else { 4.0 };
                painter.rect_filled(egui::Rect::from_center_size(at, egui::Vec2::splat(size * 2.0)), 1.0, colour);
            }
            Handle::ResizeCorner(_) => {
                let Some((at, _)) = view.project(gizmo.handle_point(handle, view)) else { continue };
                let size = if highlight(handle) { 5.0 } else { 3.5 };
                painter.circle(at, size, egui::Color32::TRANSPARENT, egui::Stroke::new(2.0_f32, colour));
            }
        }
    }
    painter.circle_filled(origin, 3.0, ui.visuals().strong_text_color());
}

/// What a snap caught: its edges as lines, its corners as small squares, and the point it landed on
/// as a hollow square (issue 87).
pub(crate) fn draw_snap_mark(painter: &egui::Painter, view: &View, mark: &crate::snap::SnapMark, colour: egui::Color32) {
    for &(a, b) in &mark.edges {
        if let (Some((a, _)), Some((b, _))) = (view.project(a), view.project(b)) {
            painter.line_segment([a, b], egui::Stroke::new(2.0_f32, colour.gamma_multiply(0.8)));
        }
    }
    for &corner in &mark.corners {
        if let Some((screen, _)) = view.project(corner) {
            painter.rect_filled(egui::Rect::from_center_size(screen, egui::Vec2::splat(6.0)), 1.0, colour);
        }
    }
    if let Some((screen, _)) = view.project(mark.at) {
        painter.rect_stroke(
            egui::Rect::from_center_size(screen, egui::Vec2::splat(11.0)),
            1.0,
            egui::Stroke::new(1.5_f32, colour),
            egui::StrokeKind::Middle,
        );
    }
}
