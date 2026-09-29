//! Finding the geometry under the pointer, and whether the camera can see it.

use super::*;
use simple3d_core::config::DisplayMode;
use simple3d_core::scene::NodeId;
use simple3d_geom::{Mesh, Vec3};

impl App {
    /// Where a measure click lands: the nearest shown snap feature within reach, else a point on the
    /// nearest line, else the surface under the pointer, else the ground; `None` on empty sky.
    /// Exact points win whenever in reach, so aiming at a corner never lands on its edge (issue 78).
    pub fn measure_point_at(&self, view: &crate::view::View, cursor: egui::Pos2) -> Option<MeasurePoint> {
        self.measure_catch(view, cursor).map(|(point, _)| point)
    }

    /// [`App::measure_point_at`], with the mark of the vertex, edge or face it caught (issue 87).
    pub fn measure_catch(
        &self,
        view: &crate::view::View,
        cursor: egui::Pos2,
    ) -> Option<(MeasurePoint, Option<crate::snap::SnapMark>)> {
        // Only what the picture shows can be caught; hidden corners and edges otherwise projected into
        // the face in front of them and snapped out of nowhere.
        let shown = |feature: &crate::snap::Feature, _: &std::sync::Arc<Mesh>| self.shows(view, feature.point);
        if let Some((feature, _, node)) = self.nearest_feature_where(view, cursor, &[], shown) {
            let point = MeasurePoint { at: feature.point, kind: Some(feature.kind) };
            return Some((point, Some(self.snap_mark(node, &feature, &[]))));
        }
        if let Some((at, kind, _, (a, b))) = self.nearest_line(view, cursor) {
            // A body edge is shown whole; axes and plane marks are drawn lines already.
            let edge = crate::snap::Feature::edge(a, b);
            let mark = (kind == crate::snap::FeatureKind::Edge).then(|| crate::snap::SnapMark::of(at, [(&edge, &[][..])]));
            return Some((MeasurePoint { at, kind: Some(kind) }, mark));
        }
        if let Some(at) = self.surface_under(view, cursor) {
            return Some((MeasurePoint { at, kind: None }, None));
        }
        let ground = view.ray_plane_ahead(cursor, Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0))?;
        Some((MeasurePoint { at: ground, kind: None }, None))
    }

    /// The nearest snap feature of a shown body within the catch radius, with its distance and body,
    /// for measuring and drag snapping; `exclude` drops the bodies being dragged.
    ///
    /// Only features facing the camera. A drag checks against the target's own body only, since the
    /// scene lags a frame and the dragged body would cover what it is aimed at.
    pub fn nearest_feature_excluding(
        &self,
        view: &crate::view::View,
        cursor: egui::Pos2,
        exclude: &[NodeId],
    ) -> Option<(crate::snap::Feature, f32, NodeId)> {
        self.nearest_feature_where(view, cursor, exclude, |feature, mesh| self.faces_the_camera(view, feature, mesh))
    }

    /// Whether a feature is on the camera's side of its own body; in wireframe, always.
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
            // A point on the surface is its own hit, so allow 0.01 mm, as `in_clear_view` does.
            Some(hit) => hit >= reach - 1e-2,
            None => true,
        }
    }

    /// The same, for a caller that accepts only some features (the measure tool). Candidates are
    /// offered nearest first, so the costly visibility check runs on as few as possible.
    pub fn nearest_feature_where(
        &self,
        view: &crate::view::View,
        cursor: egui::Pos2,
        exclude: &[NodeId],
        accept: impl Fn(&crate::snap::Feature, &std::sync::Arc<Mesh>) -> bool,
    ) -> Option<(crate::snap::Feature, f32, NodeId)> {
        let project = |p: Vec3| view.project(p).map(|(screen, _)| screen);
        let mut near: Vec<(crate::snap::Feature, f32, usize)> = Vec::new();
        let bodies = self.snap_bodies(exclude);
        for (index, (id, mesh)) in bodies.iter().enumerate() {
            let snaps = self.snaps_of(*id, mesh);
            let found = crate::snap::near_on_screen(&snaps.features, project, cursor, crate::snap::CATCH_PIXELS);
            near.extend(found.into_iter().map(|(feature, distance)| (*feature, distance, index)));
        }
        near.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        near.into_iter()
            .find(|(feature, _, index)| accept(feature, &bodies[*index].1))
            .map(|(feature, distance, index)| (feature, distance, bodies[index].0))
    }

    /// Whether nothing solid stands between the eye and a point; a point inside a body fails too.
    /// Uses the evaluated mesh the renderer draws, or the GPU's depth read-back (`gpu/depth.rs`)
    /// to avoid a ray cast and its BVH.
    pub fn in_clear_view(&self, view: &crate::view::View, at: Vec3) -> bool {
        let Some((screen, _)) = view.project(at) else { return false };
        let key = -view.to_view(at).z as f32;
        if let Some(seen) = self.gpu.as_ref().and_then(|gpu| gpu.in_sight(screen, key)) {
            return seen;
        }
        let (origin, dir) = view.ray(screen);
        let reach = (at - origin).dot(dir);
        match crate::pick::ray_mesh(&self.evaluated.mesh, origin, dir) {
            // A point on a surface is its own hit: 0.01 mm is far below measurement and above rounding.
            Some(hit) => hit >= reach - 1e-2,
            None => true,
        }
    }

    /// One body's snap features, cached between frames.
    ///
    /// Finding them costs about 6.5 ms for 16k triangles, which every visible body paid each frame.
    /// The evaluated meshes are shared `Arc`s, so the pointer is the change key.
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

    /// What snap targets depend on besides the mesh: shown axes and plane marks, and the cache key.
    pub(super) fn snap_settings(&self) -> ([bool; 3], bool, u8) {
        let axes = self.scene.settings.axes_visible;
        let marked = self.plane_marks_drawn();
        (axes, marked, (axes[0] as u8) | (axes[1] as u8) << 1 | (axes[2] as u8) << 2 | (marked as u8) << 3)
    }

    /// Whether the plane marks are on screen: enabled and not in wireframe.
    pub(super) fn plane_marks_drawn(&self) -> bool {
        self.scene.settings.plane_marks && self.settings.display_mode != DisplayMode::Wireframe
    }

    /// Whether the picture shows this point. In wireframe nothing covers anything.
    pub(super) fn shows(&self, view: &crate::view::View, at: Vec3) -> bool {
        self.settings.display_mode == DisplayMode::Wireframe || self.in_clear_view(view, at)
    }

    /// The model surface point under an interface point, or `None`. Read from the GPU picture
    /// when available, like `in_clear_view`; otherwise a ray cast.
    pub(crate) fn surface_under(&self, view: &crate::view::View, screen: egui::Pos2) -> Option<Vec3> {
        let (origin, dir) = view.ray(screen);
        if let Some(drawn) = self.gpu.as_ref().and_then(|gpu| gpu.surface_key(screen)) {
            // The key is the negated distance in front of the eye; the ray starts level with the eye.
            let key = drawn? as f64;
            return Some(origin + dir * (-key - (origin - view.eye()).dot(dir)));
        }
        crate::pick::ray_mesh(&self.evaluated.mesh, origin, dir).map(|t| origin + dir * t)
    }
}
