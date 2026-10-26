//! The viewport's pointer while the sketch is open: picking the plane, then drawing, selecting,
//! dragging and picking regions in it.

use super::picks::*;
use super::*;
use crate::app::{App, Status};
use crate::view::View;
use simple3d_core::keymap::Command;
use simple3d_geom::plane_cut::CutPlane;

/// Whether the editor takes the viewport's clicks: always while it is out.
pub(crate) fn wants_pointer(app: &App) -> bool {
    app.sketch_tool.is_some()
}

/// Whether a primary drag is the editor's rather than the camera's: one running, or one about to
/// start on a point or curve under the pointer.
pub(crate) fn owns_drag(app: &App) -> bool {
    app.sketch_tool.as_ref().is_some_and(|tool| {
        tool.drag.is_some()
            || (tool.draw == Draw::Select && tool.hover.is_some_and(|h| !matches!(h, Pick::Constraint(_))))
    })
}

pub(crate) fn interact(app: &mut App, ui: &mut egui::Ui, response: &egui::Response, view: &View) {
    let Some(tool) = app.sketch_tool.as_ref() else { return };
    let cursor = ui.input(|i| i.pointer.hover_pos()).filter(|at| response.rect.contains(*at));
    if response.hovered() {
        // The pointer only selects, so it shows the plain arrow, a hand over what it would pick.
        let icon = match (tool.draw, tool.hover) {
            (Draw::Select, Some(_)) if tool.frame.is_some() => egui::CursorIcon::PointingHand,
            (Draw::Select, None) if tool.frame.is_some() => egui::CursorIcon::Default,
            _ => egui::CursorIcon::Crosshair,
        };
        ui.ctx().set_cursor_icon(icon);
    }
    if tool.frame.is_none() {
        if response.clicked_by(egui::PointerButton::Primary) {
            if let Some(cursor) = cursor {
                pick_plane(app, view, cursor);
            }
        }
        return;
    }
    let spacing = grid_step(app, view);
    let shift = ui.input(|i| i.modifiers.shift);
    let Some(tool) = app.sketch_tool.as_mut() else { return };
    if tool.drag.is_none() {
        tool.hover = match (tool.draw, cursor) {
            (Draw::Select, Some(cursor)) => tool.pick_at(view, cursor),
            (Draw::Axis | Draw::Trim, Some(cursor)) => tool.curve_at(view, cursor).map(Pick::Curve),
            (Draw::Dimension, Some(cursor)) => tool.pick_at(view, cursor).filter(|p| !matches!(p, Pick::Constraint(_))),
            (Draw::Regions | Draw::CopyEdges, _) => None,
            (_, Some(cursor)) => tool.point_at(view, cursor, &[]).map(Pick::Point),
            _ => None,
        };
    }
    if response.clicked_by(egui::PointerButton::Secondary) {
        tool.end_gesture();
        return;
    }
    let clicked = response.clicked_by(egui::PointerButton::Primary);
    match tool.draw {
        Draw::Select => select(tool, response, view, cursor, (shift, spacing)),
        draw if !draw.draws() => {
            if let (true, Some(cursor)) = (clicked, cursor) {
                match draw {
                    Draw::Regions => app.status = toggle_region(tool, view, cursor),
                    Draw::Axis => app.status = pick_axis(tool, view, cursor),
                    Draw::Trim => app.status = trim_at(tool, view, cursor),
                    Draw::Fillet => app.status = fillet_at(tool, view, cursor),
                    Draw::Dimension => app.status = dimension_at(tool, view, cursor),
                    _ => copy_edges(app, view, cursor),
                }
            }
        }
        _ => {
            if let (true, Some(cursor)) = (response.clicked_by(egui::PointerButton::Primary), cursor) {
                if let Some(click) = tool.click_at(view, cursor, spacing) {
                    tool.click(click);
                }
            }
        }
    }
}

/// The first click: an origin plane or a face of the model puts the sketch on it, anywhere else on the
/// plane being worked on (the active workplane, or the ground). The origin planes are drawn over the
/// model, so they win over the face behind them.
pub(crate) fn pick_plane(app: &mut App, view: &View, cursor: egui::Pos2) {
    if let Some(plane) = super::origin::plane_at(view, cursor) {
        app.place_sketch(plane.frame(), None);
        return;
    }
    match app.face_under(view, cursor) {
        Some((Ok(hit), _)) => {
            let face = CutPlane { point: hit.captured.origin, normal: hit.captured.normal };
            let (u, v) = sketch_axes(face.normal);
            let frame = plane_frame(face.point, u, v);
            app.place_sketch(frame, Some(hit.owner));
        }
        Some((Err(why), _)) => app.status = Status::Warning(why.into()),
        None => {
            let frame = match app.workplane() {
                Some(plane) => plane.frame(),
                None => Xform::from_translation(Vec3::ZERO),
            };
            app.place_sketch(frame, None);
        }
    }
}

/// The grid step clicks and drags snap to: the finest level drawn, as in other CAD programs without
/// a key held, so none while the grid is hidden. A point under the pointer still wins over it.
pub(crate) fn grid_step(app: &App, view: &View) -> Option<f64> {
    app.ground_grid(view).map(|grid| grid.spacing).filter(|step| *step > 0.0)
}

/// The axes of a sketch on a face facing `normal`: its X level, so that looking straight at it, as
/// the camera does without rolling, puts X to the right and Y up. Only a face looking straight up or
/// down has no level direction of its own; there X is the world's, turned to keep the frame
/// right-handed as seen from the face's side, as the origin planes are.
pub(crate) fn sketch_axes(normal: Vec3) -> (Vec3, Vec3) {
    let n = normal.normalized();
    let u = if n.z.abs() > 0.999 {
        Vec3::new(n.z.signum(), 0.0, 0.0)
    } else {
        Vec3::new(0.0, 0.0, 1.0).cross(n).normalized()
    };
    (u, n.cross(u))
}

/// A frame with origin `origin` and axes `u`, `v` and their normal.
pub(crate) fn plane_frame(origin: Vec3, u: Vec3, v: Vec3) -> Xform {
    let rotation = Xform::rotation_of_axes(u, v, u.cross(v));
    Xform::from_pos_rot(origin, rotation)
}

/// Selecting by click and moving by drag.
fn select(
    tool: &mut SketchTool,
    response: &egui::Response,
    view: &View,
    cursor: Option<egui::Pos2>,
    (shift, step): (bool, Option<f64>),
) {
    let Some(cursor) = cursor else { return };
    if response.drag_started_by(egui::PointerButton::Primary) {
        if let Some(pick @ (Pick::Point(_) | Pick::Curve(_))) = tool.hover {
            if !tool.selection.contains(&pick) {
                if !shift {
                    tool.selection.clear();
                }
                tool.selection.push(pick);
            }
            start_drag(tool, view, cursor);
        }
    }
    if let Some(drag) = tool.drag.as_ref() {
        if let Some(at) = tool.under(view, cursor) {
            let delta = snapped_delta(drag, [at[0] - drag.start[0], at[1] - drag.start[1]], step);
            let moves: Vec<(PointId, [f64; 2])> =
                drag.points.iter().map(|&(id, from)| (id, [from[0] + delta[0], from[1] + delta[1]])).collect();
            tool.sketch.drag(&moves);
        }
        if !response.dragged() {
            let drag = tool.drag.take().expect("checked above");
            if tool.sketch != drag.before {
                tool.remember(drag.before);
                tool.refresh();
            }
        }
        return;
    }
    if response.clicked_by(egui::PointerButton::Primary) {
        match tool.hover {
            Some(pick) if shift => match tool.selection.iter().position(|p| *p == pick) {
                Some(at) => {
                    tool.selection.remove(at);
                }
                None => tool.selection.push(pick),
            },
            // A plain click also opens the popup that changes what was clicked.
            Some(pick) => {
                tool.selection = vec![pick];
                tool.open_inspect(pick, cursor);
            }
            None if !shift => {
                tool.selection.clear();
                tool.inspect = None;
            }
            None => {}
        }
    }
}

/// How far a drag moves its points: as far as the pointer, or with a grid step, as far as puts the
/// point nearest where the drag began onto the grid, so the shape lands on it and keeps its form.
pub(crate) fn snapped_delta(drag: &Dragging, delta: [f64; 2], step: Option<f64>) -> [f64; 2] {
    let Some(step) = step else { return delta };
    let gap = |p: [f64; 2]| (p[0] - drag.start[0]).hypot(p[1] - drag.start[1]);
    let Some(&(_, from)) = drag.points.iter().min_by(|a, b| gap(a.1).total_cmp(&gap(b.1))) else { return delta };
    let to = [from[0] + delta[0], from[1] + delta[1]].map(|v| (v / step).round() * step);
    [to[0] - from[0], to[1] - from[1]]
}

/// Start dragging every selected point and the points of every selected curve.
fn start_drag(tool: &mut SketchTool, view: &View, cursor: egui::Pos2) {
    let Some(start) = tool.under(view, cursor) else { return };
    let mut ids: Vec<PointId> = Vec::new();
    for pick in &tool.selection {
        match *pick {
            Pick::Point(id) => ids.push(id),
            Pick::Curve(id) => ids.extend(tool.sketch.curve(id).map(|c| c.shape.points()).unwrap_or_default()),
            Pick::Constraint(_) => {}
        }
    }
    ids.sort_unstable();
    ids.dedup();
    let points = ids
        .into_iter()
        .filter_map(|id| tool.sketch.point(id).filter(|p| !p.fixed).map(|p| (id, p.at)))
        .collect::<Vec<_>>();
    if points.is_empty() {
        return;
    }
    tool.drag = Some(Dragging { start, points, before: tool.sketch.clone() });
}

impl App {
    /// A command while the sketch is open, if the editor takes it: undo and redo step its own
    /// history, delete removes what is selected in it. `true` when taken.
    pub(crate) fn run_in_sketch(&mut self, command: Command) -> bool {
        let Some(tool) = self.sketch_tool.as_mut() else { return false };
        match command {
            Command::Undo => tool.undo_step(),
            Command::Redo => tool.redo_step(),
            Command::Delete => tool.delete_selection(),
            _ => return false,
        }
        true
    }
}

impl SketchTool {
    /// Remove every selected constraint, curve and point.
    pub(crate) fn delete_selection(&mut self) {
        let selection = std::mem::take(&mut self.selection);
        self.change(|sketch| {
            for pick in &selection {
                match *pick {
                    Pick::Constraint(id) => sketch.remove_constraint(id),
                    Pick::Curve(id) => sketch.remove_curve(id),
                    Pick::Point(id) => sketch.remove_point(id),
                }
            }
        });
    }
}
