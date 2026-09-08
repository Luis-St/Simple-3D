//! Moving a drag onto what it snapped to.

use super::*;
use crate::gizmo::{Gizmo, Handle};
use simple3d_core::scene::NodeId;
use simple3d_geom::Vec3;

impl App {
    /// Snap the dragged node so one of its own features lands on the nearest
    /// feature of another body under the pointer (issue 68). Returns the world
    /// point it snapped onto, or `None` when nothing was in reach, in which case
    /// the grid drag stands.
    ///
    /// `handle` is what the drag is being steered by, and the snap stays inside
    /// it: an axis handle only ever moves along its axis and a plane handle only
    /// within its plane. Writing the full three-dimensional correction turned an
    /// X-axis drag into a free move -- the body jumped in Y and Z as well, which
    /// is the one thing choosing an axis handle says it must not do.
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
        // Only the components the handle actually governs survive, measured in
        // the handle's own frame rather than the world's so a rotated body's
        // local axes are respected the same way the drag itself respects them.
        let constrain = |wanted: Vec3| {
            let mut correction = Vec3::ZERO;
            for axis in handle.axes() {
                let dir = gizmo.axes[axis];
                correction = correction + dir * wanted.dot(dir);
            }
            correction
        };
        // Where every feature the carried body offers currently is.
        let sources: Vec<Vec3> =
            std::iter::once(Vec3::ZERO).chain(self.snap_sources.iter().copied()).map(|o| world_origin + o).collect();
        // Brought alongside first, aimed at second: the drag catches on whatever
        // the body has come near, and only when it has come near nothing does the
        // feature the pointer is over get its say.
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

    /// The snap a drag catches by bringing the body alongside another (issue 68):
    /// the least the body can be moved, within what the handle allows, to put one
    /// of its own features onto a feature of a body it is not carrying.
    ///
    /// This is what makes snapping reachable at all. Taking the target from
    /// whatever the *pointer* is over cannot place two bodies against each other:
    /// the handle is grabbed some seventy pixels out from the body, so by the time
    /// the pointer reaches the corner to meet, the body has already been dragged
    /// on top of it -- two 20 mm boxes could be snapped into the same 20 mm of
    /// space and into nothing else. What a person is actually judging as they drag
    /// is whether the thing they are carrying has come alongside the thing they
    /// want it against, which is the question asked here.
    ///
    /// Nearness is judged on screen, where the judgement is being made, so a snap
    /// takes the same aim whatever the zoom. Candidates are bucketed by screen
    /// cell rather than compared all against all: a body of any size offers a
    /// feature per corner and per edge, and a drag asks this on every frame.
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
        // Every pair near enough on screen to be meant, cheapest move first. The
        // whole list rather than the single best, because the winner still has to
        // be a feature the picture shows -- and that question costs a ray cast, so
        // it is asked of the pairs in the order they would be taken rather than of
        // all of them.
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

    /// The snap a drag catches by being aimed: the feature under the pointer, and
    /// whichever of the carried body's own features the handle lets reach it.
    ///
    /// The fallback to [`App::snap_alongside`], and the one that can cross a gap:
    /// nothing has to have come near anything, so this is how a body is thrown
    /// onto a corner some way off. An empty group offers no features of its own,
    /// and then it is the origin that lands on the target, which still beats
    /// refusing to snap at all.
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
            // How far the feature still misses the target after the constrained
            // move: exactly zero when it can reach, and the shortest achievable
            // gap when the handle will not let it all the way there.
            let miss = (*source + correction - target.point).length();
            // Several features often reach equally well -- on an X drag towards a
            // corner, the box's left face can meet it just as exactly as its
            // right, by flying the whole body past the target and landing on top
            // of it. Between equals, the one that moves the body least is the one
            // that was meant.
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
