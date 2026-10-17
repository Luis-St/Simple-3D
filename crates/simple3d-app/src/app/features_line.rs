//! The nearest point on a line, and whether a drag is asking to snap.

use super::*;
use crate::snap::FeatureKind;
use simple3d_core::config::SnapMode;
use simple3d_core::keymap::Command;
use simple3d_core::scene::NodeId;
use simple3d_geom::Vec3;

impl App {
    /// The point on the nearest line (a body edge, a plane mark, or a world axis) for a pointer near
    /// one but not near a notable point on it (issue 78), leaving out the bodies of `exclude`. Edges
    /// come from the feature list; axes and plane marks are measurable lines in their own right. Also
    /// returns the line, so a caught edge can be shown whole (issue 87).
    pub(crate) fn nearest_line(
        &self,
        view: &crate::view::View,
        cursor: egui::Pos2,
        exclude: &[NodeId],
    ) -> Option<(Vec3, FeatureKind, f32, (Vec3, Vec3))> {
        let mut lines = Vec::new();
        for (id, mesh) in self.snap_bodies(exclude) {
            let snaps = self.snaps_of(id, &mesh);
            lines.extend(
                snaps.features.iter().filter_map(|feature| feature.span).map(|(a, b)| (a, b, FeatureKind::Edge)),
            );
            // The principal-plane marks on the surface, which are read for exactly this kind of measurement.
            lines.extend(snaps.marks.iter().map(|&(a, b)| (a, b, FeatureKind::PlaneMark)));
        }
        // Only as far as the axes are drawn.
        let reach = crate::render::grid_radius(view);
        let axes = crate::snap::axis_lines(self.scene.settings.axes_visible, reach);
        lines.extend(axes.into_iter().map(|(a, b)| (a, b, FeatureKind::Axis)));
        self.nearest_on_lines(view, cursor, lines, exclude)
    }

    /// The nearest point in reach and in sight on any of `lines`, with its kind, screen distance and line.
    pub(crate) fn nearest_on_lines(
        &self,
        view: &crate::view::View,
        cursor: egui::Pos2,
        lines: impl IntoIterator<Item = (Vec3, Vec3, FeatureKind)>,
        exclude: &[NodeId],
    ) -> Option<(Vec3, FeatureKind, f32, (Vec3, Vec3))> {
        let project = |p: Vec3| view.project(p).map(|(screen, _)| screen);
        let mut near: Vec<(Vec3, FeatureKind, f32, (Vec3, Vec3))> = Vec::new();
        for (a, b, kind) in lines {
            if let Some((at, distance)) = crate::snap::nearest_on_edge(a, b, project, cursor, crate::snap::CATCH_PIXELS)
            {
                near.push((at, kind, distance, (a, b)));
            }
        }
        // Nearest first, and the nearest visible one wins; hidden edges, marks and axis stretches once
        // made the tool jump to lines inside the object.
        near.sort_by(|a, b| a.2.total_cmp(&b.2));
        near.into_iter().find(|&(at, kind, _, _)| self.sees(view, at, kind, exclude))
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
    /// faces, from the snap bodies ([`App::snap_bodies`]). Gathered once per evaluation.
    pub(crate) fn model_edges(&self) -> std::rc::Rc<Vec<(Vec3, Vec3)>> {
        let key = std::sync::Arc::as_ptr(&self.evaluated.mesh) as usize;
        if let Some((held, edges)) = self.model_edges.borrow().as_ref() {
            if *held == key {
                return edges.clone();
            }
        }
        let edges: Vec<(Vec3, Vec3)> = self
            .snap_bodies(&[])
            .iter()
            .flat_map(|(id, mesh)| {
                self.snaps_of(*id, mesh).features.iter().filter_map(|feature| feature.span).collect::<Vec<_>>()
            })
            .collect();
        let edges = std::rc::Rc::new(edges);
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
