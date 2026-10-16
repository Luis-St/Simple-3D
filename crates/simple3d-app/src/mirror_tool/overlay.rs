//! What the tool draws: mirror images in the picture, and the line and plane over it.

use super::*;
use crate::app::App;
use crate::theme::token;
use crate::view::View;
use simple3d_core::xform::Xform;

/// Each object's mirror image, as the object and the reflection carrying it there, for the renderer to
/// draw translucent. Empty while choosing, or while the line is missing.
pub(crate) fn templates(app: &App) -> Vec<(NodeId, Xform)> {
    let Some(tool) = app.mirror_tool.as_ref().filter(|tool| !tool.choosing) else { return Vec::new() };
    let Ok((point, normal)) = tool.plane() else { return Vec::new() };
    let mirror = Xform::reflection(point, normal);
    tool.targets.iter().map(|&id| (id, mirror)).collect()
}

/// The line, the plane's outline around the objects and their images, and what a click would pick.
/// Drawn over the picture, so a line behind a body stays readable.
pub(crate) fn draw(app: &App, ui: &egui::Ui, painter: &egui::Painter, view: &View) {
    let Some(tool) = app.mirror_tool.as_ref().filter(|tool| !tool.choosing) else { return };
    let colour = token::ACCENT;
    let screen = |p: Vec3| view.project(p).map(|(at, _)| at);
    let line = |a: Vec3, b: Vec3, width: f32, colour: egui::Color32| {
        if let (Some(a), Some(b)) = (screen(a), screen(b)) {
            painter.line_segment([a, b], egui::Stroke::new(width, colour));
        }
    };
    if let (Ok((point, along)), Ok((_, normal))) = (tool.line(), tool.plane()) {
        let reach = reach(app, tool, point, along, normal);
        let across = normal.cross(along).normalized();
        let ((lo, hi), (down, up)) = reach;
        // The plane as a faint frame around everything mirrored, the line itself solid through it.
        let corner = |s: f64, t: f64| point + along * s + across * t;
        let frame = [corner(lo, down), corner(hi, down), corner(hi, up), corner(lo, up)];
        for i in 0..4 {
            line(frame[i], frame[(i + 1) % 4], 1.0, colour.gamma_multiply(0.45));
        }
        line(corner(lo, 0.0), corner(hi, 0.0), 2.0, colour);
    }
    for end in tool.ends.iter().flatten() {
        if let Some(at) = screen(*end) {
            painter.rect_filled(egui::Rect::from_center_size(at, egui::Vec2::splat(5.0)), 1.0, colour);
        }
    }

    // What a click would pick, only over the viewport itself.
    let Some(cursor) = ui.input(|i| i.pointer.hover_pos()).filter(|at| painter.clip_rect().contains(*at)) else {
        return;
    };
    match tool.by {
        MirrorBy::Edge => {
            if let Some((a, b)) = app.nearest_model_edge(view, cursor) {
                line(a, b, 3.0, colour.gamma_multiply(0.6));
            }
        }
        MirrorBy::Line => {
            let Some((hover, caught)) = app.measure_catch(view, cursor) else { return };
            match &caught {
                Some(caught) => crate::panel_viewport::draw_snap_mark(painter, view, caught, colour),
                None => {
                    if let Some(at) = screen(hover.at) {
                        painter.circle_stroke(at, 4.0, egui::Stroke::new(1.5_f32, colour));
                    }
                }
            }
            // A rubber band from the start, to aim the end.
            if let [Some(start), None] = tool.ends {
                line(start, hover.at, 1.0, colour.gamma_multiply(0.6));
            }
        }
        MirrorBy::Axis => {}
    }
}

/// How far something runs along the line, then across it, from the line's point.
type Reach = ((f64, f64), (f64, f64));

/// How far the plane's frame runs along the line and across it from `point`: over the objects, their
/// images and the line's own ends, with a margin.
fn reach(app: &App, tool: &MirrorTool, point: Vec3, along: Vec3, normal: Vec3) -> Reach {
    let across = normal.cross(along).normalized();
    let mirror = Xform::reflection(point, normal);
    let mut corners: Vec<Vec3> = tool.ends.iter().flatten().copied().collect();
    for id in &tool.targets {
        let Some(&(lo, hi)) = app.evaluated.node_world_bounds.get(id) else { continue };
        for i in 0..8 {
            let corner = Vec3::new(
                if i & 1 == 0 { lo.x } else { hi.x },
                if i & 2 == 0 { lo.y } else { hi.y },
                if i & 4 == 0 { lo.z } else { hi.z },
            );
            corners.extend([corner, mirror.point(corner)]);
        }
    }
    let spans = |direction: Vec3| {
        corners
            .iter()
            .map(|&c| (c - point).dot(direction))
            .fold((0.0_f64, 0.0_f64), |(lo, hi), d| (lo.min(d), hi.max(d)))
    };
    let ((lo, hi), (down, up)) = (spans(along), spans(across));
    let margin = 0.1 * (hi - lo).max(up - down).max(10.0);
    ((lo - margin, hi + margin), (down - margin, up + margin))
}
