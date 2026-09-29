//! The nearest point on a line, and whether a drag is asking to snap.

use super::*;
use simple3d_core::config::SnapMode;
use simple3d_core::keymap::Command;
use simple3d_geom::Vec3;

impl App {
    /// The point on the nearest line (a body edge, a plane mark, or a world axis) for a pointer near
    /// one but not near a notable point on it (issue 78). Edges come from the feature list; axes and
    /// plane marks are measurable lines in their own right. Also returns the line, so a caught
    /// edge can be shown whole (issue 87).
    pub(super) fn nearest_line(
        &self,
        view: &crate::view::View,
        cursor: egui::Pos2,
    ) -> Option<(Vec3, crate::snap::FeatureKind, f32, (Vec3, Vec3))> {
        let project = |p: Vec3| view.project(p).map(|(screen, _)| screen);
        let mut near: Vec<(Vec3, crate::snap::FeatureKind, f32, (Vec3, Vec3))> = Vec::new();
        let mut consider = |a: Vec3, b: Vec3, kind: crate::snap::FeatureKind| {
            if let Some((at, distance)) = crate::snap::nearest_on_edge(a, b, project, cursor, crate::snap::CATCH_PIXELS)
            {
                near.push((at, kind, distance, (a, b)));
            }
        };
        for (&id, mesh) in &self.evaluated.node_meshes {
            if !self.scene.is_shown(id) {
                continue;
            }
            let snaps = self.snaps_of(id, mesh);
            for feature in &snaps.features {
                if let Some((a, b)) = feature.span {
                    consider(a, b, crate::snap::FeatureKind::Edge);
                }
            }
            // The principal-plane marks on the surface, which are read for exactly this kind of measurement.
            for &(a, b) in &snaps.marks {
                consider(a, b, crate::snap::FeatureKind::PlaneMark);
            }
        }
        // Only as far as the axes are drawn.
        let reach = crate::render::grid_radius(view);
        for (a, b) in crate::snap::axis_lines(self.scene.settings.axes_visible, reach) {
            consider(a, b, crate::snap::FeatureKind::Axis);
        }
        // Nearest first, and the nearest visible one wins; hidden edges, marks and axis stretches once
        // made the tool jump to lines inside the object.
        near.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
        near.into_iter().find(|&(at, kind, _, _)| match kind {
            // Axes keep their own visibility test in every mode, since their stretch inside a body is cut
            // out even in wireframe.
            crate::snap::FeatureKind::Axis => self.in_clear_view(view, at),
            _ => self.shows(view, at),
        })
    }

    /// The model edge nearest the pointer and in sight, for picking a path (issue 70). Only real
    /// edges: axes and plane marks are not part of the model's outline.
    pub(crate) fn nearest_model_edge(&self, view: &crate::view::View, cursor: egui::Pos2) -> Option<(Vec3, Vec3)> {
        let project = |p: Vec3| view.project(p).map(|(screen, _)| screen);
        let mut near: Vec<(Vec3, f32, (Vec3, Vec3))> = Vec::new();
        for &(a, b) in self.model_edges().iter() {
            if let Some((at, distance)) = crate::snap::nearest_on_edge(a, b, project, cursor, crate::snap::CATCH_PIXELS)
            {
                near.push((at, distance, (a, b)));
            }
        }
        near.sort_by(|a, b| a.1.total_cmp(&b.1));
        near.into_iter().find(|&(at, ..)| self.shows(view, at)).map(|(.., edge)| edge)
    }

    /// Every real edge of the model as drawn: creases and open borders, not the diagonals of flat
    /// faces. The evaluated result rather than each shape's own mesh, since a boolean's operands have
    /// edges the result does not: a cutter's, and those of what it cut away. Found once per evaluation.
    pub(crate) fn model_edges(&self) -> std::rc::Rc<Vec<(Vec3, Vec3)>> {
        let key = std::sync::Arc::as_ptr(&self.evaluated.mesh) as usize;
        if let Some((held, edges)) = self.model_edges.borrow().as_ref() {
            if *held == key {
                return edges.clone();
            }
        }
        let (features, _) = crate::snap::features_and_faces(&self.evaluated.mesh);
        let edges = std::rc::Rc::new(features.iter().filter_map(|feature| feature.span).collect::<Vec<_>>());
        *self.model_edges.borrow_mut() = Some((key, edges.clone()));
        edges
    }

    /// Whether geometry snapping is requested now (issue 68): always, never, or while the key is held.
    /// `key_down` reports toolkit keys, which only the viewport can see.
    pub fn geometry_snap_wanted(&self, key_down: impl Fn(egui::Key) -> bool, mods: egui::Modifiers) -> bool {
        match self.settings.geometry_snap {
            SnapMode::Never => false,
            SnapMode::Always => true,
            SnapMode::WhileHeld => self.holding(Command::SnapToGeometry, key_down, mods),
        }
    }

    /// Whether Ctrl is the snap key (the default, issue 77); then it wins over Ctrl's symmetric resize.
    pub fn snap_holds_ctrl(&self) -> bool {
        self.settings.geometry_snap == SnapMode::WhileHeld
            && self.keymap.binding(Command::SnapToGeometry).is_some_and(|chord| chord.ctrl)
    }
}
