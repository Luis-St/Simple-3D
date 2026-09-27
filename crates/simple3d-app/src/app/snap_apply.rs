//! Moving a drag onto what it snapped to.

use super::*;
use crate::gizmo::{Gizmo, Handle};
use simple3d_core::scene::NodeId;
use simple3d_geom::Vec3;

impl App {
    /// Snap the dragged node so one of its features lands on another body's feature under the pointer
    /// (issue 68). Returns the snapped point, or `None` to keep the grid drag. The snap stays within
    /// what `handle` allows, so an axis drag never moves off its axis.
    pub(super) fn apply_geometry_snap(
        &mut self,
        id: NodeId,
        gizmo: &Gizmo,
        handle: Handle,
        view: &crate::view::View,
        cursor: egui::Pos2,
    ) -> Option<Vec3> {
        let frame = *self.evaluated.node_frames.get(&id)?;
        let world_origin = frame.point(self.scene.node(id).position);
        let exclude = self.drag_subtree(id);
        // Keep only the components the handle governs, in the handle's frame so rotated axes are respected.
        let constrain = |wanted: Vec3| {
            let mut correction = Vec3::ZERO;
            for axis in handle.axes() {
                let dir = gizmo.axes[axis];
                correction = correction + dir * wanted.dot(dir);
            }
            correction
        };
        // Where every feature of the carried body currently is.
        let sources: Vec<Vec3> = std::iter::once(Vec3::ZERO)
            .chain(self.drag_feature_offsets().iter().copied())
            .map(|o| world_origin + o)
            .collect();
        // Alongside first; only when the body is near nothing does the feature under the pointer count.
        let (correction, target) = self
            .snap_alongside(view, &sources, &exclude, &constrain)
            .or_else(|| self.snap_at_pointer(view, cursor, &sources, &exclude, &constrain))?;
        let new_origin = world_origin + correction;
        let new_position = frame.inverse().point(new_origin);
        if let Some(node) = self.scene.get_mut(id) {
            node.position = new_position;
        }
        Some(target)
    }

    /// The snap caught by bringing the body alongside another (issue 68): the least constrained move
    /// putting one of its features on another body's feature.
    ///
    /// Needed because the handle sits far out from the body, so pointer-based targets would overlap
    /// the bodies. Nearness is judged on screen, zoom-independently, with candidates bucketed by cell.
    pub(super) fn snap_alongside(
        &self,
        view: &crate::view::View,
        sources: &[Vec3],
        exclude: &[NodeId],
        constrain: &impl Fn(Vec3) -> Vec3,
    ) -> Option<(Vec3, Vec3)> {
        let cell = crate::snap::DRAG_CATCH_PIXELS;
        let mut buckets: std::collections::HashMap<(i32, i32), Vec<usize>> = std::collections::HashMap::new();
        let mut screens: Vec<Option<egui::Pos2>> = Vec::with_capacity(sources.len());
        for (index, point) in sources.iter().enumerate() {
            let screen = view.project(*point).map(|(at, _)| at);
            if let Some(at) = screen {
                buckets.entry(((at.x / cell).floor() as i32, (at.y / cell).floor() as i32)).or_default().push(index);
            }
            screens.push(screen);
        }
        // All near pairs, cheapest first, since the winner must also be visible, which costs a ray cast.
        let mut pairs: Vec<(f64, Vec3, crate::snap::Feature, NodeId)> = Vec::new();
        for (&node, mesh) in &self.evaluated.node_meshes {
            if !self.scene.is_shown(node) || exclude.contains(&node) {
                continue;
            }
            for feature in self.snaps_of(node, mesh).features.iter() {
                let Some((at, _)) = view.project(feature.point) else { continue };
                let (cx, cy) = ((at.x / cell).floor() as i32, (at.y / cell).floor() as i32);
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        let Some(near) = buckets.get(&(cx + dx, cy + dy)) else { continue };
                        for &index in near {
                            let Some(source_at) = screens[index] else { continue };
                            if (source_at - at).length() > cell {
                                continue;
                            }
                            let correction = constrain(feature.point - sources[index]);
                            pairs.push((correction.length(), correction, *feature, node));
                        }
                    }
                }
            }
        }
        pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        pairs
            .into_iter()
            .find(|(_, _, feature, node)| {
                self.evaluated.node_meshes.get(node).is_some_and(|mesh| self.faces_the_camera(view, feature, mesh))
            })
            .map(|(_, correction, feature, _)| (correction, feature.point))
    }

    /// The snap caught by aiming: the feature under the pointer and whichever carried feature the
    /// handle lets reach it. The fallback to [`App::snap_alongside`], able to cross gaps; an empty
    /// group snaps its origin.
    pub(super) fn snap_at_pointer(
        &self,
        view: &crate::view::View,
        cursor: egui::Pos2,
        sources: &[Vec3],
        exclude: &[NodeId],
        constrain: &impl Fn(Vec3) -> Vec3,
    ) -> Option<(Vec3, Vec3)> {
        let (target, _) = self.nearest_feature_excluding(view, cursor, exclude)?;
        let mut best: Option<(Vec3, f64, f64)> = None;
        for source in sources {
            let correction = constrain(target.point - *source);
            // The remaining miss after the constrained move: zero when reachable.
            let miss = (*source + correction - target.point).length();
            // Among equally good features, prefer the least travel, not flying the body past the target.
            let travel = correction.length();
            const TIE: f64 = 1e-6;
            let better = match best {
                None => true,
                Some((_, best_miss, best_travel)) => {
                    miss < best_miss - TIE || ((miss - best_miss).abs() <= TIE && travel < best_travel)
                }
            };
            if better {
                best = Some((correction, miss, travel));
            }
        }
        best.map(|(correction, _, _)| (correction, target.point))
    }
}
