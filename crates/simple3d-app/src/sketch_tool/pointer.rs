//! The viewport's pointer while the sketch is open: picking the plane, then drawing, selecting,
//! dragging and picking regions in it.

use super::*;
use crate::app::{App, Status};
use crate::view::View;
use simple3d_core::keymap::Command;
use simple3d_geom::plane_cut::CutPlane;
use simple3d_sketch::Shape;

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
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }
    if tool.frame.is_none() {
        if response.clicked_by(egui::PointerButton::Primary) {
            if let Some(cursor) = cursor {
                pick_plane(app, view, cursor);
            }
        }
        return;
    }
    let spacing = if app.snap_requested { app.ground_grid(view).map(|grid| grid.spacing) } else { None };
    let shift = ui.input(|i| i.modifiers.shift);
    let Some(tool) = app.sketch_tool.as_mut() else { return };
    if tool.drag.is_none() {
        tool.hover = match (tool.draw, cursor) {
            (Draw::Select, Some(cursor)) => tool.pick_at(view, cursor),
            (Draw::Axis, Some(cursor)) => tool.curve_at(view, cursor).map(Pick::Curve),
            (_, Some(cursor)) => tool.point_at(view, cursor, &[]).map(Pick::Point),
            _ => None,
        };
    }
    if response.clicked_by(egui::PointerButton::Secondary) {
        tool.end_gesture();
        return;
    }
    match tool.draw {
        Draw::Select => select(tool, response, view, cursor, shift),
        Draw::Regions | Draw::Axis | Draw::CopyEdges => {
            if let (true, Some(cursor)) = (response.clicked_by(egui::PointerButton::Primary), cursor) {
                let draw = tool.draw;
                match draw {
                    Draw::Regions => app.status = toggle_region(tool, view, cursor),
                    Draw::Axis => app.status = pick_axis(tool, view, cursor),
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
            let (u, v) = face.axes();
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

/// A frame with origin `origin` and axes `u`, `v` and their normal.
pub(crate) fn plane_frame(origin: Vec3, u: Vec3, v: Vec3) -> Xform {
    let rotation = Xform::rotation_of_axes(u, v, u.cross(v));
    Xform::from_pos_rot(origin, rotation)
}

/// Selecting by click and moving by drag.
fn select(tool: &mut SketchTool, response: &egui::Response, view: &View, cursor: Option<egui::Pos2>, shift: bool) {
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
            let delta = [at[0] - drag.start[0], at[1] - drag.start[1]];
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
            Some(pick) => tool.selection = vec![pick],
            None if !shift => tool.selection.clear(),
            None => {}
        }
    }
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

/// Pick or drop the region under the pointer.
fn toggle_region(tool: &mut SketchTool, view: &View, cursor: egui::Pos2) -> Status {
    let Some(at) = tool.under(view, cursor) else { return Status::Info(String::new()) };
    let regions = simple3d_sketch::regions::find(&tool.sketch, 64);
    let Some(index) = regions.at(at) else {
        return Status::Info("No closed region there; close the outline first".into());
    };
    let held = tool.sketch.regions.iter().position(|pick| regions.resolve(pick) == Some(index));
    let pick = regions.pick(index, at);
    tool.change(|sketch| match held {
        Some(k) => {
            sketch.regions.remove(k);
        }
        None => sketch.regions.push(pick),
    });
    Status::Info(if held.is_some() { "Dropped the region" } else { "Picked the region" }.into())
}

/// Make the line under the pointer the revolve axis.
fn pick_axis(tool: &mut SketchTool, view: &View, cursor: egui::Pos2) -> Status {
    match tool.curve_at(view, cursor) {
        Some(id) if tool.sketch.curve(id).is_some_and(|c| matches!(c.shape, Shape::Line { .. })) => {
            tool.change(|sketch| sketch.axis = Some(id));
            tool.params.insert("feature".into(), simple3d_core::primitive::ParamValue::Choice(1));
            Status::Info("The solid now revolves about that line".into())
        }
        _ => Status::Info("Click a line to revolve about".into()),
    }
}

/// Copy the edges of the face under the pointer onto the sketch, dropped onto its plane.
fn copy_edges(app: &mut App, view: &View, cursor: egui::Pos2) {
    let hit = match app.face_under(view, cursor) {
        Some((Ok(hit), _)) => hit,
        Some((Err(why), _)) => {
            app.status = Status::Warning(why.into());
            return;
        }
        None => {
            app.status = Status::Info("Click a face of the model to copy its edges".into());
            return;
        }
    };
    let Some(tool) = app.sketch_tool.as_mut() else { return };
    let face = hit.captured.clone();
    let helper = tool.copy_as_helper;
    let to_sketch = |p: [f64; 2]| tool.local(face.origin + face.u * p[0] + face.v * p[1]);
    let loops: Vec<Vec<[f64; 2]>> = std::iter::once(&face.outline.outer)
        .chain(face.outline.holes.iter())
        .map(|points| points.iter().map(|&p| to_sketch(p)).collect())
        .collect();
    let added = tool.change(|sketch| {
        let mut added = 0;
        for points in &loops {
            for i in 0..points.len() {
                added +=
                    sketch.add_reference_line(points[i], points[(i + 1) % points.len()], helper).is_some() as usize;
            }
        }
        added
    });
    app.status = Status::Info(format!("Copied {added} edges of the face"));
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
