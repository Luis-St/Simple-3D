//! The nearest point on an edge, and when a drag is asking to snap at all.

use super::*;
use simple3d_core::config::SnapMode;
use simple3d_core::keymap::Command;
use simple3d_geom::Vec3;

impl App {
    /// The point on the nearest *line* -- a body's edge, the mark a principal
    /// plane leaves across it, or a world axis -- for a pointer that is near one
    /// but not near any of the notable points on it (issue 78).
    ///
    /// Edges come from the same feature list, which carries each edge's two ends
    /// beside its midpoint, so they need no second pass over the geometry. The
    /// axes are lines in their own right: a point on one is as real a place to
    /// measure from as a corner is, and offering only the handful of places
    /// where an axis meets something left the rest of it -- most of it -- with
    /// nothing to catch. The plane marks are the same argument on the surface:
    /// where the axis itself runs through the material and is not drawn, the
    /// mark is what the picture puts there, and it is what a pointer over the
    /// body is aiming at.
    pub fn nearest_line_point(
        &self,
        view: &crate::view::View,
        cursor: egui::Pos2,
    ) -> Option<(Vec3, crate::snap::FeatureKind, f32)> {
        let project = |p: Vec3| view.project(p).map(|(screen, _)| screen);
        let mut near: Vec<(Vec3, crate::snap::FeatureKind, f32)> = Vec::new();
        let mut consider = |a: Vec3, b: Vec3, kind: crate::snap::FeatureKind| {
            if let Some((at, distance)) = crate::snap::nearest_on_edge(a, b, project, cursor, crate::snap::CATCH_PIXELS)
            {
                near.push((at, kind, distance));
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
            // The marks the principal planes leave on the surface. They are
            // lines on the body, drawn in the colour of the axis whose plane
            // made them, and a measurement along one -- how far along this face
            // is the plane through zero -- is exactly what they are read for.
            for &(a, b) in &snaps.marks {
                consider(a, b, crate::snap::FeatureKind::PlaneMark);
            }
        }
        // As far as the axes are actually drawn, so nothing is caught out where
        // there is no line to see.
        let reach = crate::render::grid_radius(view);
        for (a, b) in crate::snap::axis_lines(self.scene.settings.axes_visible, reach) {
            consider(a, b, crate::snap::FeatureKind::Axis);
        }
        // Nearest first, and the nearest one the picture actually shows wins.
        //
        // Every one of these is a line that stops where the drawing stops. An
        // axis is cut out of the material it runs through and covered by whatever
        // is in front of it; an edge on the far side of a solid, and a plane mark
        // on the back of one, are behind that solid however near the pointer
        // their projection lands. Catching them anyway is what made the tool jump
        // to lines inside the object, which is the one thing no line on screen
        // does.
        near.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
        near.into_iter().find(|&(at, kind, _)| match kind {
            // The axis keeps its own question in every display mode: wireframe
            // fills nothing and hides nothing, but the stretch inside a body is
            // still cut out of the line there.
            crate::snap::FeatureKind::Axis => self.in_clear_view(view, at),
            _ => self.shows(view, at),
        })
    }

    /// Whether geometry snapping is being asked for right now (issue 68): always,
    /// never, or only while the snap key is held. `key_down` answers whether a
    /// toolkit key is currently pressed, which only the viewport can see.
    pub fn geometry_snap_wanted(&self, key_down: impl Fn(egui::Key) -> bool, mods: egui::Modifiers) -> bool {
        match self.settings.geometry_snap {
            SnapMode::Never => false,
            SnapMode::Always => true,
            SnapMode::WhileHeld => self.holding(Command::SnapToGeometry, key_down, mods),
        }
    }
}
