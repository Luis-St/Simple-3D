//! Finding the geometry under the pointer, and whether it can be seen from
//! where the camera stands.

use super::*;
use simple3d_core::config::DisplayMode;
use simple3d_core::scene::NodeId;
use simple3d_geom::{Mesh, Vec3};

impl App {
    /// Where a measure click lands: the nearest snap feature of any shown body if
    /// one is within reach on screen, otherwise a point along the nearest line --
    /// an edge, a plane mark, an axis -- otherwise the point on the surface under
    /// the pointer, otherwise the ground plane. `None` only when the pointer is
    /// on empty sky, where there is nothing to measure to.
    ///
    /// The order is what makes placement predictable (issue 78): the exact
    /// points -- corners, midpoints, axis crossings -- win whenever one is in
    /// reach, and a line catches the pointer only where none of them does, so
    /// aiming at a corner never lands part-way along the edge beside it.
    pub fn measure_point_at(&self, view: &crate::view::View, cursor: egui::Pos2) -> Option<MeasurePoint> {
        // Only what is on screen can be caught. A feature the model covers is not
        // being pointed at: the corner is around the back of the solid, or the
        // edge is its far bottom one, and neither is drawn -- but both project
        // into the middle of the face in front of them, where a pointer aimed at
        // that face snapped to them out of nowhere. So the same question the axes
        // have always been asked, "does the picture show this?", is asked of
        // every feature.
        let shown = |feature: &crate::snap::Feature, _: &std::sync::Arc<Mesh>| self.shows(view, feature.point);
        if let Some((feature, _)) = self.nearest_feature_where(view, cursor, &[], shown) {
            return Some(MeasurePoint { at: feature.point, kind: Some(feature.kind) });
        }
        if let Some((at, kind, _)) = self.nearest_line_point(view, cursor) {
            return Some(MeasurePoint { at, kind: Some(kind) });
        }
        if let Some(at) = self.surface_under(view, cursor) {
            return Some(MeasurePoint { at, kind: None });
        }
        view.ray_plane_ahead(cursor, Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)).map(|at| MeasurePoint { at, kind: None })
    }

    /// The snap feature of a shown body nearest the cursor on screen, within the
    /// catch radius. Shared by the measure tool and by geometry snapping during a
    /// drag; `exclude` drops the bodies a drag is itself moving so it never snaps
    /// to the very thing it is carrying.
    ///
    /// Only the features the body actually turns towards the camera: a corner
    /// round the back of a solid is not one anybody is aiming at, however near
    /// the pointer its projection lands, and a drag that jumped onto one moved
    /// the body for no reason the picture gave. The measure tool asks the same
    /// question of the whole scene; a drag asks it of the target's own body
    /// only, because the scene it would have to ask describes where the dragged
    /// body *was* -- `Evaluated` lags a drag by a frame -- and the body being
    /// carried would spend the whole gesture covering the very thing it is being
    /// aimed at.
    pub fn nearest_feature_excluding(
        &self,
        view: &crate::view::View,
        cursor: egui::Pos2,
        exclude: &[NodeId],
    ) -> Option<(crate::snap::Feature, f32)> {
        self.nearest_feature_where(view, cursor, exclude, |feature, mesh| self.faces_the_camera(view, feature, mesh))
    }

    /// Whether a feature is on the side of its own body that the camera can see.
    ///
    /// Wireframe fills nothing and hides nothing, so there every feature is as
    /// catchable as the line that shows it.
    pub(super) fn faces_the_camera(
        &self,
        view: &crate::view::View,
        feature: &crate::snap::Feature,
        mesh: &std::sync::Arc<Mesh>,
    ) -> bool {
        if self.settings.display_mode == DisplayMode::Wireframe {
            return true;
        }
        let Some((screen, _)) = view.project(feature.point) else { return false };
        let (origin, dir) = view.ray(screen);
        let reach = (feature.point - origin).dot(dir);
        match crate::pick::ray_mesh(mesh, origin, dir) {
            // A point on the surface is its own hit, so the comparison leaves
            // room for one -- the same hundredth of a millimetre `in_clear_view`
            // allows, far below anything a placement cares about.
            Some(hit) => hit >= reach - 1e-2,
            None => true,
        }
    }

    /// The same, for a caller that will not take every feature -- the measure
    /// tool, which takes only the ones the picture shows.
    ///
    /// The candidates within reach are ranked by screen distance and offered
    /// outward from the cursor, so the first one accepted is the nearest
    /// acceptable one. Ranking rather than filtering is what makes that cheap:
    /// "does the frame show this?" costs a ray cast through the scene, and asking
    /// it of the feature under the pointer is one cast where asking it of every
    /// feature of every body is hundreds.
    pub fn nearest_feature_where(
        &self,
        view: &crate::view::View,
        cursor: egui::Pos2,
        exclude: &[NodeId],
        accept: impl Fn(&crate::snap::Feature, &std::sync::Arc<Mesh>) -> bool,
    ) -> Option<(crate::snap::Feature, f32)> {
        let project = |p: Vec3| view.project(p).map(|(screen, _)| screen);
        let mut near: Vec<(crate::snap::Feature, f32, NodeId)> = Vec::new();
        for (&id, mesh) in &self.evaluated.node_meshes {
            if !self.scene.is_shown(id) || exclude.contains(&id) {
                continue;
            }
            let snaps = self.snaps_of(id, mesh);
            let found = crate::snap::near_on_screen(&snaps.features, project, cursor, crate::snap::CATCH_PIXELS);
            near.extend(found.into_iter().map(|(feature, distance)| (*feature, distance, id)));
        }
        near.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        near.into_iter()
            .find(|(feature, _, id)| self.evaluated.node_meshes.get(id).is_some_and(|mesh| accept(feature, mesh)))
            .map(|(feature, distance, _)| (feature, distance))
    }

    /// Whether a point can be seen from where the camera is: nothing solid
    /// between the eye and it.
    ///
    /// The evaluated mesh is exactly what the renderer draws as material, and
    /// what it cuts the axis lines out of, so asking it is asking the same
    /// question the picture answers. A point *inside* a body fails too, since
    /// the body's own near surface is in front of it.
    ///
    /// Read off the picture itself when the GPU is drawing it (`gpu/depth.rs`)
    /// -- the same question, answered without a ray cast through the whole
    /// scene, and without the tree of every triangle in it a cast needs.
    pub fn in_clear_view(&self, view: &crate::view::View, at: Vec3) -> bool {
        let Some((screen, _)) = view.project(at) else { return false };
        let key = -view.to_view(at).z as f32;
        if let Some(seen) = self.gpu.as_ref().and_then(|gpu| gpu.in_sight(screen, key)) {
            return seen;
        }
        let (origin, dir) = view.ray(screen);
        let reach = (at - origin).dot(dir);
        match crate::pick::ray_mesh(&self.evaluated.mesh, origin, dir) {
            // A point on a surface is its own hit, so the comparison has to
            // leave room for one: a hundredth of a millimetre is far below
            // anything a measurement cares about and far above the arithmetic.
            Some(hit) => hit >= reach - 1e-2,
            None => true,
        }
    }

    /// One body's snap features, remembered between frames.
    ///
    /// Finding them welds the mesh, builds two hash maps over its edges, runs a
    /// union-find across its coplanar triangles and sorts the result -- 6.5 ms
    /// for a 16k-triangle body in a release build. Every visible body was paying
    /// that on *every frame* of a snapped drag and of a measure hover, which is
    /// most of a frame's budget spent recomputing something that only changes
    /// when the mesh does. The evaluated meshes are shared `Arc`s, so the
    /// pointer is exactly the "has this changed" key: a re-evaluation makes a new
    /// allocation and misses, and anything else hits.
    pub(super) fn snaps_of(&self, id: NodeId, mesh: &std::sync::Arc<simple3d_geom::Mesh>) -> Snaps {
        let (axes, marked, mask) = self.snap_settings();
        let key = (std::sync::Arc::as_ptr(mesh) as usize, mask);
        if let Some((cached_key, snaps)) = self.snap_features.borrow().get(&id) {
            if *cached_key == key {
                return snaps.clone();
            }
        }
        let snaps = std::rc::Rc::new(find_snaps(mesh, axes, marked));
        self.snap_features.borrow_mut().insert(id, (key, snaps.clone()));
        snaps
    }

    /// What a body's snap targets depend on besides its mesh: which axes are
    /// shown, whether the plane marks are, and the two as the cache's key.
    pub(super) fn snap_settings(&self) -> ([bool; 3], bool, u8) {
        let axes = self.scene.settings.axes_visible;
        let marked = self.plane_marks_drawn();
        (axes, marked, (axes[0] as u8) | (axes[1] as u8) << 1 | (axes[2] as u8) << 2 | (marked as u8) << 3)
    }

    /// Whether the plane marks are on screen: the switch for them, and a display
    /// mode with a surface for them to sit on. They are not drawn in wireframe,
    /// so there is nothing there to catch either.
    pub(super) fn plane_marks_drawn(&self) -> bool {
        self.scene.settings.plane_marks && self.settings.display_mode != DisplayMode::Wireframe
    }

    /// Whether the picture shows this point, or material stands in front of it.
    ///
    /// What the measure tool takes is what the frame shows -- a corner on the far
    /// side of a solid is not a corner anybody is pointing at, however near the
    /// pointer its projection lands. Wireframe fills nothing, so nothing there
    /// covers anything: the far side of a body is drawn exactly like the near
    /// side, which is the point of that mode, and every feature of it is as
    /// catchable as the line that shows it.
    pub(super) fn shows(&self, view: &crate::view::View, at: Vec3) -> bool {
        self.settings.display_mode == DisplayMode::Wireframe || self.in_clear_view(view, at)
    }

    /// The point of the model's surface under a point of the interface, or
    /// `None` where there is none.
    ///
    /// Read off the picture when the GPU is drawing it, like `in_clear_view`;
    /// otherwise a ray cast through the evaluated scene.
    pub(crate) fn surface_under(&self, view: &crate::view::View, screen: egui::Pos2) -> Option<Vec3> {
        let (origin, dir) = view.ray(screen);
        if let Some(drawn) = self.gpu.as_ref().and_then(|gpu| gpu.surface_key(screen)) {
            // The key is the distance in front of the eye, negated; the ray
            // starts level with the eye.
            let key = drawn? as f64;
            return Some(origin + dir * (-key - (origin - view.eye()).dot(dir)));
        }
        crate::pick::ray_mesh(&self.evaluated.mesh, origin, dir).map(|t| origin + dir * t)
    }
}
