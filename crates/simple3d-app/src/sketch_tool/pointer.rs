//! The viewport's pointer while the sketch is open: picking the plane, then drawing, selecting,
//! dragging and picking regions in it.

use super::picks::*;
use super::*;
use crate::app::App;
use crate::view::View;
use simple3d_core::keymap::Command;

/// Whether the editor takes the viewport's clicks: always while it is out.
pub(crate) fn wants_pointer(app: &App) -> bool {
    app.sketch_tool.is_some()
}

/// Whether a primary drag is the editor's rather than the camera's: one running, or one about to
/// start on a point or curve under the pointer.
pub(crate) fn owns_drag(app: &App) -> bool {
    app.sketch_tool.as_ref().is_some_and(|tool| {
        tool.drag.is_some()
            || tool.area.is_some()
            || tool.label_drag.is_some()
            || (tool.draw == Draw::Select
                && tool.hover.is_some_and(|h| match h {
                    Pick::Constraint(id) => tool.sketch.constraint(id).is_some_and(|c| c.kind.value().is_some()),
                    _ => true,
                }))
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
    let (shift, mods) = ui.input(|i| (i.modifiers.shift, i.modifiers));
    // A left drag over empty space selects by area, unless the camera is bound to it.
    let camera_drag =
        crate::panel_viewport::nav_gesture(&app.keymap.nav, [true, false, false], mods.command, mods.shift, mods.alt)
            .is_some();
    let origin = ui.input(|i| i.pointer.press_origin()).filter(|_| !camera_drag);
    let Some(tool) = app.sketch_tool.as_mut() else { return };
    tool.free = mods.command;
    if tool.drag.is_none() && tool.label_drag.is_none() {
        tool.hover = cursor.and_then(|cursor| tool.hover_at(view, cursor, shift));
    }
    if response.clicked_by(egui::PointerButton::Secondary) {
        tool.end_gesture();
        return;
    }
    let clicked = response.clicked_by(egui::PointerButton::Primary);
    match tool.draw {
        Draw::Select => {
            if !tool.area_select(response, view, cursor, (origin, shift)) {
                select(tool, response, view, cursor, (shift, spacing));
            }
        }
        draw if !draw.draws() => {
            if let (true, Some(cursor)) = (clicked, cursor) {
                match draw {
                    Draw::Regions => app.status = toggle_region(tool, view, cursor),
                    Draw::Axis => app.status = pick_axis(tool, view, cursor),
                    Draw::Trim => app.status = trim_at(tool, view, cursor),
                    Draw::Fillet => app.status = fillet_at(tool, view, cursor),
                    Draw::Chamfer => app.status = super::picks_more::chamfer_at(tool, view, cursor),
                    Draw::Extend | Draw::Split | Draw::Offset => {
                        app.status = super::picks_more::curve_tool_at(tool, view, cursor)
                    }
                    draw if draw.uses_selection() => {
                        app.status = super::picks_more::selection_tool_at(tool, view, cursor, (shift, spacing))
                    }
                    Draw::Dimension => {
                        let (status, typed) = super::dimension::dimension_at(tool, view, cursor);
                        app.status = status;
                        if let Some(id) = typed {
                            let at = super::dimension_draw::all(tool, view)
                                .into_iter()
                                .find(|(d, _)| *d == id)
                                .map_or(cursor, |(_, shape)| shape.at);
                            tool.open_inspect(Pick::Constraint(id), at);
                            app.fields.open_typing(super::inspect_constraint::value_field(id));
                        }
                    }
                    Draw::Constrain(rule) => app.status = super::constrain_pick::constrain_at(tool, view, cursor, rule),
                    _ => copy_edges(app, view, cursor),
                }
            }
        }
        _ => {
            if let (true, Some(cursor)) = (response.clicked_by(egui::PointerButton::Primary), cursor) {
                if let Some((click, inferred)) = tool.drawing_click(view, cursor, spacing) {
                    tool.click_inferred(click, inferred);
                }
            }
        }
    }
}

/// The grid step clicks and drags snap to: the level drawn, only while snapping is asked for (the snap
/// key held, as for every other snap), so none without it or while the grid is hidden. A point under
/// the pointer still wins over it.
pub(crate) fn grid_step(app: &App, view: &View) -> Option<f64> {
    if !app.snap_requested {
        return None;
    }
    app.ground_grid(view).map(|grid| grid.spacing).filter(|step| *step > 0.0)
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
    if super::label_drag::drag_label(tool, response, view, cursor) {
        return;
    }
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
            // Shift marks without the popup, and closes one left open, which would only show one of them.
            Some(pick) if shift => {
                match tool.selection.iter().position(|p| *p == pick) {
                    Some(at) => {
                        tool.selection.remove(at);
                    }
                    None => tool.selection.push(pick),
                }
                tool.inspect = None;
            }
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
    let Some(from) = drag.anchor() else { return delta };
    let to = [from[0] + delta[0], from[1] + delta[1]].map(|v| (v / step).round() * step);
    [to[0] - from[0], to[1] - from[1]]
}

impl Dragging {
    /// Where the dragged point nearest where the drag began started out: the one a snapped drag puts
    /// onto the grid.
    pub(crate) fn anchor(&self) -> Option<[f64; 2]> {
        let gap = |p: [f64; 2]| (p[0] - self.start[0]).hypot(p[1] - self.start[1]);
        self.points.iter().min_by(|a, b| gap(a.1).total_cmp(&gap(b.1))).map(|&(_, from)| from)
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
