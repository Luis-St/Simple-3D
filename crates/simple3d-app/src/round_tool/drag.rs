//! Sizing the treatment by dragging an edge: the pointer's travel along one of its faces, away from
//! the edge, is how far back along the faces the treatment reaches.

use super::*;
use crate::view::View;
use simple3d_geom::Vec3;

/// An edge being dragged.
#[derive(Clone)]
pub(crate) struct SizeDrag {
    pub(crate) edge: FeatureEdge,
    /// The face direction the travel is read along: whichever of the two shows longer on screen.
    along: Vec3,
    /// Where on the edge the drag took hold, and where on the screen.
    grab: Vec3,
    press: egui::Pos2,
    /// The size before the drag, for Escape.
    pub(crate) before: f64,
}

impl App {
    /// Take hold of the edge under `press`, picking it when it is not yet. `false` when there is none.
    pub(crate) fn start_round_drag(&mut self, view: &View, press: egui::Pos2) -> bool {
        let Some(pick::Pick::Edge(edge)) = self.round_pick_at(view, press) else { return false };
        let screen = |p: Vec3| view.project(p).map(|(s, _)| s);
        let reach = f32::INFINITY;
        let Some((grab, _)) = crate::snap::nearest_on_edge(edge.a, edge.b, screen, press, reach) else { return false };
        let shown = |dir: Vec3| match (screen(grab), screen(grab + dir)) {
            (Some(a), Some(b)) => (b - a).length(),
            _ => 0.0,
        };
        let along = if shown(edge.along[0]) >= shown(edge.along[1]) { edge.along[0] } else { edge.along[1] };
        let Some(tool) = self.round_tool.as_mut() else { return false };
        if !tool.edges.iter().any(|e| pick::same_edge(e, &edge)) {
            tool.edges.push(edge);
        }
        tool.drag = Some(SizeDrag { edge, along, grab, press, before: tool.size() });
        true
    }

    /// Size the treatment for the pointer at `cursor`, snapped to the move step.
    pub(crate) fn drag_round_size(&mut self, view: &View, cursor: egui::Pos2) {
        let step = self.move_snap();
        let Some(tool) = self.round_tool.as_mut() else { return };
        let Some(drag) = tool.drag.as_ref() else { return };
        let ends = (view.project(drag.grab), view.project(drag.grab + drag.along));
        let (Some((from, _)), Some((to, _))) = ends else { return };
        let along = to - from;
        if along.length_sq() < 1e-6 {
            return;
        }
        let reach = ((cursor - drag.press).dot(along) / along.length_sq()) as f64;
        // A round reaches further than its radius along the faces of a sharp edge.
        let size = match tool.kind {
            Kind::Round => reach * (drag.edge.opening() / 2.0).tan(),
            Kind::Chamfer => reach,
        };
        // Never below one step, so a drag back over the edge still leaves a treatment to see.
        tool.set_size(((size / step).round() * step).max(step));
    }
}
