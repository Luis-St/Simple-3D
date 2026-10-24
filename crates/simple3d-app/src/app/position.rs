//! Picking a position under the pointer (issue 126): the one path every tool that puts a point down
//! goes through -- the measure tool, the drawn lines of the mirror and arrange tools, the 3D cursor
//! and putting a new shape down.
//!
//! While snapping is asked for ([`App::snap_requested`]: the snap key held, or snapping always on) a
//! pick catches, nearest first within each stage: a body's corners, edge midpoints and face centres;
//! then its edges and plane marks, and the world axes; then the ground grid's crossings; then its
//! lines. Otherwise, or with nothing in reach, it lands on the surface under the pointer, else the
//! ground.

use super::*;
use crate::render::GridPlane;
use crate::snap::{Feature, FeatureKind, GroundGrid, SnapMark};
use crate::view::View;
use simple3d_core::config::DisplayMode;
use simple3d_core::scene::NodeId;
use simple3d_geom::{Mesh, Vec3};
use std::sync::Arc;

/// A picked position, with the mark of the vertex, edge or face it caught, if any (issue 87).
pub(crate) type Picked = (MeasurePoint, Option<SnapMark>);

impl App {
    /// Where a pick at `cursor` lands, leaving `exclude` (and what is in it) out of what can be caught
    /// or landed on; `None` on empty sky.
    pub fn pick_position(&self, view: &View, cursor: egui::Pos2, exclude: &[NodeId]) -> Option<Picked> {
        self.pick_snapping(view, cursor, exclude, self.snap_requested)
    }

    /// [`App::pick_position`], snapping or not regardless of what is asked for now.
    pub(crate) fn pick_snapping(
        &self,
        view: &View,
        cursor: egui::Pos2,
        exclude: &[NodeId],
        snap: bool,
    ) -> Option<Picked> {
        if snap {
            if let Some(caught) = self.snap_catch(view, cursor, exclude) {
                return Some(caught);
            }
        }
        let surface = if exclude.is_empty() {
            self.surface_under(view, cursor)
        } else {
            self.surface_hit_under(view, cursor, exclude).map(|(at, _, _)| at)
        };
        let at = surface.or_else(|| self.work_ground_under(view, cursor))?;
        Some((MeasurePoint { at, kind: None }, None))
    }

    /// What a snapping pick catches, whether or not snapping is asked for now; `None` with nothing in
    /// reach. Exact points win whenever in reach, so aiming at a corner never lands on its edge (issue 78).
    pub fn snap_catch(&self, view: &View, cursor: egui::Pos2, exclude: &[NodeId]) -> Option<Picked> {
        // Only what the picture shows can be caught; hidden corners and edges otherwise projected into
        // the face in front of them and snapped out of nowhere.
        let shown = |feature: &Feature, _: &Arc<Mesh>| self.sees(view, feature.point, feature.kind, exclude);
        if let Some((feature, _, node)) = self.nearest_feature_where(view, cursor, exclude, shown) {
            let point = MeasurePoint { at: feature.point, kind: Some(feature.kind) };
            return Some((point, Some(self.snap_mark(node, &feature, &[]))));
        }
        let (points, lines) = self
            .ground_grid(view)
            .and_then(|grid| Some((grid, grid.plane.under(view, cursor)?)))
            .map_or_else(|| (Vec::new(), Vec::new()), |(grid, ground)| grid.near(ground));
        if let Some((at, kind, _, (a, b))) = self.nearest_line(view, cursor, exclude) {
            // An axis is a grid line too, so near one of its grid points the point wins.
            let on_line = |p: &Vec3| (b - a).cross(*p - a).length() <= 1e-9 * (b - a).length().max(1.0);
            let on_axis: Vec<Vec3> = points.iter().copied().filter(on_line).collect();
            if kind == FeatureKind::Axis {
                if let Some(point) = self.nearest_grid_point(view, cursor, &on_axis, exclude) {
                    return Some(point);
                }
            }
            // A body edge is shown whole; axes and plane marks are drawn lines already.
            let edge = Feature::edge(a, b);
            let mark = (kind == FeatureKind::Edge).then(|| SnapMark::of(at, [(&edge, &[][..])]));
            return Some((MeasurePoint { at, kind: Some(kind) }, mark));
        }
        if let Some(point) = self.nearest_grid_point(view, cursor, &points, exclude) {
            return Some(point);
        }
        let lines = lines.into_iter().map(|(a, b)| (a, b, FeatureKind::GridLine));
        let (at, kind, _, _) = self.nearest_on_lines(view, cursor, lines, exclude)?;
        Some((MeasurePoint { at, kind: Some(kind) }, None))
    }

    /// The nearest grid crossing in reach and in sight, marked as the point it is.
    fn nearest_grid_point(
        &self,
        view: &View,
        cursor: egui::Pos2,
        points: &[Vec3],
        exclude: &[NodeId],
    ) -> Option<Picked> {
        let features: Vec<Feature> = points.iter().map(|&p| Feature::point(p, FeatureKind::GridPoint)).collect();
        let project = |p: Vec3| view.project(p).map(|(screen, _)| screen);
        let mut near = crate::snap::near_on_screen(&features, project, cursor, crate::snap::CATCH_PIXELS);
        near.sort_by(|a, b| a.1.total_cmp(&b.1));
        let (feature, _) = near.into_iter().find(|(f, _)| self.sees(view, f.point, f.kind, exclude))?;
        Some((MeasurePoint { at: feature.point, kind: Some(feature.kind) }, Some(SnapMark::at(feature.point))))
    }

    /// The grid as drawn at this zoom, on the ground or the active workplane (issue 125), or `None`
    /// while it is hidden. The level snapped to is the one the eye reads: the finer once it has mostly
    /// faded in.
    pub(crate) fn ground_grid(&self, view: &View) -> Option<GroundGrid> {
        if !self.scene.settings.grid_visible {
            return None;
        }
        let plane = self.grid_plane().unwrap_or_else(|| GridPlane::ground(self.scene.settings.grid_spacing));
        let (fine, coarse, strength) = crate::render::grid_levels_on(view, &plane);
        let spacing = if strength >= 0.5 { fine } else { coarse };
        let radius = crate::render::grid_radius_on(view, &plane);
        let (centre, half) = crate::render::grid_square_on(view, &plane, spacing, radius);
        Some(GroundGrid { plane, spacing, centre, half })
    }

    /// Whether a pick can see this point. With nothing left out, whether the picture shows it; axes
    /// keep their own test in every mode, since their stretch inside a body is cut out even in
    /// wireframe. With bodies left out, which cover what they are aimed at in the picture, whether
    /// any other body stands in front of it.
    pub(crate) fn sees(&self, view: &View, at: Vec3, kind: FeatureKind, exclude: &[NodeId]) -> bool {
        if exclude.is_empty() {
            return if kind == FeatureKind::Axis { self.in_clear_view(view, at) } else { self.shows(view, at) };
        }
        if self.settings.display_mode == DisplayMode::Wireframe && kind != FeatureKind::Axis {
            return true;
        }
        let Some((screen, _)) = view.project(at) else { return false };
        let (origin, dir) = view.ray(screen);
        let reach = (at - origin).dot(dir);
        // A point on a surface is its own hit, so allow 0.01 mm, as `in_clear_view` does. One ray through
        // the drawn model, not one per body, since this runs for every snap position in reach.
        self.surface_hit_under(view, screen, exclude).is_none_or(|(hit, _, _)| (hit - origin).dot(dir) >= reach - 1e-2)
    }

    /// The model surface under the pointer with its triangle and its normal facing the eye, passing
    /// through `exclude` and everything in them: what is being put down or arranged, which would
    /// otherwise always be hit first.
    pub(crate) fn surface_hit_under(
        &self,
        view: &View,
        cursor: egui::Pos2,
        exclude: &[NodeId],
    ) -> Option<(Vec3, usize, Vec3)> {
        let (origin, dir) = view.ray(cursor);
        crate::pick::surface_hit(&self.evaluated.mesh, origin, dir, |source| {
            let owner = NodeId::from(source);
            exclude.iter().any(|&id| id == owner || self.scene.is_ancestor_of(id, owner))
        })
    }
}
