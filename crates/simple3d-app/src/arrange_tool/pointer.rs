//! The viewport's clicks while a path is being made: picking edges or drawing points.

use super::*;
use crate::app::{App, Status};
use crate::view::View;
use simple3d_geom::path::smooth_run;

/// Whether the tool takes the viewport's clicks: only while spreading along a path.
pub(crate) fn wants_pointer(app: &App) -> bool {
    app.arrange_tool.as_ref().is_some_and(|tool| tool.mode == Arrange::Path)
}

/// A click adds to the path, a right-click (not a right drag, which orbits) takes the last addition
/// back. Selecting is left alone, since the tool works on the objects it opened on.
pub(crate) fn interact(app: &mut App, ui: &mut egui::Ui, response: &egui::Response, view: &View) {
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }
    if response.clicked_by(egui::PointerButton::Secondary) {
        take_back(app);
        return;
    }
    if !response.clicked_by(egui::PointerButton::Primary) {
        return;
    }
    let Some(cursor) = ui.input(|i| i.pointer.interact_pos()) else { return };
    let Some(source) = app.arrange_tool.as_ref().map(|tool| tool.source) else { return };
    match source {
        PathSource::Edges => {
            let Some(run) = run_under(app, view, cursor) else {
                app.status = Status::Info("Click on a body's edge to add it to the path".into());
                return;
            };
            let Some(tool) = app.arrange_tool.as_mut() else { return };
            // A picked edge clicked again is taken out, with the rest of its click.
            let picked = tool.runs.iter().position(|held| held.iter().any(|&edge| same_edge(edge, run[0])));
            match picked {
                Some(at) => {
                    tool.runs.remove(at);
                }
                None => tool.runs.push(run),
            }
        }
        PathSource::Drawn => {
            let Some((point, _)) = app.measure_catch(view, cursor) else { return };
            if let Some(tool) = app.arrange_tool.as_mut() {
                tool.points.push(point.at);
            }
        }
    }
}

/// The edges a click at `cursor` would pick: the model's edge itself, or the smooth run it belongs to.
pub(crate) fn run_under(app: &App, view: &View, cursor: egui::Pos2) -> Option<Vec<(Vec3, Vec3)>> {
    let edge = app.nearest_model_edge(view, cursor)?;
    let whole = app.arrange_tool.as_ref().is_some_and(|tool| tool.whole_run);
    Some(if whole { smooth_run(&app.model_edges(), edge) } else { vec![edge] })
}

fn same_edge((a, b): (Vec3, Vec3), (c, d): (Vec3, Vec3)) -> bool {
    let near = |p: Vec3, q: Vec3| (p - q).length() <= simple3d_geom::path::SAME_POINT;
    (near(a, c) && near(b, d)) || (near(a, d) && near(b, c))
}

/// Take the last click back: its edges, or its point.
pub(crate) fn take_back(app: &mut App) {
    let Some(tool) = app.arrange_tool.as_mut() else { return };
    let taken = match tool.source {
        PathSource::Edges => tool.runs.pop().is_some(),
        PathSource::Drawn => tool.points.pop().is_some(),
    };
    if !taken {
        app.status = Status::Info("The path is empty; nothing to take back".into());
    }
}
