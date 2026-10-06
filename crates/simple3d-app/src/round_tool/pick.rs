//! Picking edges and corners in the viewport, and dragging one to size the treatment.

use super::*;
use crate::view::View;
use simple3d_geom::Vec3;

/// What a click would pick.
#[derive(Clone, Debug)]
pub(crate) enum Pick {
    Edge(FeatureEdge),
    Corner(Corner),
    /// An inside corner ([`simple3d_geom::rounding::inside_corners`]).
    Joint(Vec3),
}

/// The tool's pointer: a click picks or drops the edge or corner under it, a drag on an edge sizes
/// the treatment, a right-click takes the last pick back, Escape lets go of a drag or closes the tool.
/// Selecting is left alone while the tool is out.
pub(crate) fn interact(app: &mut App, ui: &mut egui::Ui, response: &egui::Response, view: &View) {
    let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
    if let Some(tool) = app.round_tool.as_mut().filter(|tool| tool.drag.is_some()) {
        if escape {
            let drag = tool.drag.take().expect("checked above");
            tool.set_size(drag.before);
            app.status = Status::Info("Sizing cancelled".into());
            return;
        }
        if let Some(cursor) = ui.input(|i| i.pointer.hover_pos()) {
            app.drag_round_size(view, cursor);
        }
        if response.drag_stopped() || ui.input(|i| i.pointer.any_released()) {
            if let Some(tool) = app.round_tool.as_mut() {
                tool.drag = None;
            }
        }
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        return;
    }
    if escape {
        app.cancel_round_tool();
        app.status = Status::Info("Round and bevel closed".into());
        return;
    }
    if response.drag_started_by(egui::PointerButton::Primary) {
        let press = ui.input(|i| i.pointer.press_origin().or(i.pointer.hover_pos()));
        if press.is_some_and(|press| app.start_round_drag(view, press)) {
            return;
        }
    }
    // The picker everywhere, edges included: picking is what a click does.
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }
    if response.clicked_by(egui::PointerButton::Secondary) {
        let Some(tool) = app.round_tool.as_mut() else { return };
        // The most recent pick is whichever list grew last; corners are rarer, so edges go last.
        if tool.corners.pop().is_none() && tool.joints.pop().is_none() && tool.edges.pop().is_none() {
            app.status = Status::Info("Nothing picked to take back".into());
        }
        return;
    }
    if !response.clicked_by(egui::PointerButton::Primary) {
        return;
    }
    let Some(cursor) = ui.input(|i| i.pointer.interact_pos()) else { return };
    let Some(pick) = app.round_pick_at(view, cursor) else {
        app.status = Status::Info("Click on an edge or a corner of the model".into());
        return;
    };
    let Some(tool) = app.round_tool.as_mut() else { return };
    match pick {
        Pick::Edge(edge) => match tool.edges.iter().position(|e| same_edge(e, &edge)) {
            Some(at) => {
                tool.edges.remove(at);
            }
            None => tool.edges.push(edge),
        },
        Pick::Corner(corner) => match tool.corners.iter().position(|c| near(c.at, corner.at)) {
            Some(at) => {
                tool.corners.remove(at);
            }
            None => tool.corners.push(corner),
        },
        Pick::Joint(at) => match tool.joints.iter().position(|&j| near(j, at)) {
            Some(index) => {
                tool.joints.remove(index);
            }
            None => tool.joints.push(at),
        },
    }
}

pub(crate) fn near(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < 1e-4
}

pub(crate) fn same_edge(p: &FeatureEdge, q: &FeatureEdge) -> bool {
    (near(p.a, q.a) && near(p.b, q.b)) || (near(p.a, q.b) && near(p.b, q.a))
}

impl App {
    /// The shown corner or edge nearest `cursor`, inside corners first, then corners, within the catch
    /// radius. A pick already made counts as shown: the draft may have covered it with a fillet or cut
    /// it away.
    pub(crate) fn round_pick_at(&self, view: &View, cursor: egui::Pos2) -> Option<Pick> {
        let features = self.round_features()?;
        let tool = self.round_tool.as_ref()?;
        let reach = crate::snap::CATCH_PIXELS;
        let screen = |p: Vec3| view.project(p).map(|(s, _)| s);
        // An inside corner first: the tip of the corner standing between two earlier roundings is a
        // corner of the model too, but rounding that alone is not what picking there means.
        let joint = features
            .joints
            .iter()
            .filter_map(|&j| screen(j).map(|s| (j, s.distance(cursor))))
            .filter(|&(j, d)| d <= reach && (tool.joints.iter().any(|&p| near(p, j)) || self.shows(view, j)))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((joint, _)) = joint {
            return Some(Pick::Joint(joint));
        }
        let corner = features
            .corners
            .iter()
            .filter_map(|c| screen(c.at).map(|s| (c, s.distance(cursor))))
            .filter(|(c, d)| *d <= reach && (tool.corners.iter().any(|p| near(p.at, c.at)) || self.shows(view, c.at)))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((corner, _)) = corner {
            return Some(Pick::Corner(corner.clone()));
        }
        let mut best: Option<(f32, &FeatureEdge)> = None;
        for edge in &features.edges {
            let Some((at, distance)) = crate::snap::nearest_on_edge(edge.a, edge.b, screen, cursor, reach) else {
                continue;
            };
            if best.is_some_and(|(d, _)| d <= distance) {
                continue;
            }
            if !tool.edges.iter().any(|e| same_edge(e, edge)) && !self.shows(view, at) {
                continue;
            }
            best = Some((distance, edge));
        }
        best.map(|(_, edge)| Pick::Edge(*edge))
    }
}
