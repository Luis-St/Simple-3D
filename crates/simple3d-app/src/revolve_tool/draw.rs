//! What the tool draws: the profile it would take or has taken, the axis, the turn at the pointer,
//! and the solid itself in the picture where the bodies in front can hide it.

use super::*;
use crate::app::App;
use crate::raster::Rgba;
use crate::render::{Palette, Renderable};
use crate::theme::token;
use crate::view::View;
use simple3d_core::xform::Xform;

pub(crate) fn draw(app: &App, ui: &egui::Ui, painter: &egui::Painter, view: &View) {
    if app.mode != crate::gizmo::Mode::Revolve {
        return;
    }
    let cursor = ui.input(|i| i.pointer.hover_pos()).filter(|at| painter.clip_rect().contains(*at));
    let tool = &app.revolve;
    let turning = tool.preview.as_ref().is_some_and(|(_, made)| made.is_ok());
    if let Some(profile) = &tool.profile {
        if !turning {
            crate::push_pull_tool::draw::tint_target(app, painter, view, profile, 0.25, true);
        }
    }
    if let Some(axis) = &tool.axis {
        line(painter, view, axis.world, 2.0);
    }
    // What a click would take: a line to turn about, or another profile.
    match cursor.and_then(|cursor| super::interact::under(app, view, cursor)) {
        Some(super::interact::Under::Axis(axis)) => line(painter, view, axis.world, 1.25),
        Some(super::interact::Under::Profile(target)) => {
            crate::push_pull_tool::draw::tint_target(app, painter, view, &target, 0.18, true);
        }
        _ => {}
    }
}

/// The axis as a dashed line, run on past its ends so a short edge still reads as a line.
fn line(painter: &egui::Painter, view: &View, (a, b): (Vec3, Vec3), width: f32) {
    let d = b - a;
    let ends = [a - d * 0.25, b + d * 0.25].map(|p| view.project(p).map(|(s, _)| s));
    if let [Some(p), Some(q)] = ends {
        painter.extend(egui::Shape::dashed_line(&[p, q], egui::Stroke::new(width, token::ACCENT), 8.0, 5.0));
    }
}

/// The turned solid as a template for the renderer: green where it adds, red where it cuts.
/// Takes the tool alone, as the picture is drawn while the app's other fields are borrowed.
pub(crate) fn template<'a>(tool: &'a Revolve, palette: &Palette) -> Option<(&'a Renderable, Xform, Option<Rgba>)> {
    let (_, Ok(solid)) = tool.preview.as_ref()? else { return None };
    Some((&**solid, Xform::IDENTITY, Some(if tool.cuts() { palette.reduction } else { palette.extrusion })))
}
