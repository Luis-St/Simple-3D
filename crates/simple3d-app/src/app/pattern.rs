//! Patterns: their handles, and making one from a selection.

use super::*;
use crate::gizmo::{self};
use simple3d_core::scene::NodeId;
use simple3d_geom::Vec3;

impl App {
    /// Every lay-out grip of pattern `id`, in world space (issue 67); none for non-patterns or mirrors.
    pub fn pattern_grips(&self, id: NodeId) -> Vec<PatternGrip> {
        let Some(node) = self.scene.get(id) else { return Vec::new() };
        if !node.is_pattern() {
            return Vec::new();
        }
        let Some(params) = node.params() else { return Vec::new() };
        let Some(gizmo) = self.gizmo_for(id) else { return Vec::new() };
        let own = gizmo.own;
        let centre = self.pattern_content_centre(id, &own);
        simple3d_core::pattern::grips(params)
            .into_iter()
            .map(|grip| on_the_copies(grip, centre))
            .map(|grip| {
                let along = own.vector(grip.dir);
                let scale = along.length().max(1e-9);
                let turn = match grip.drive {
                    simple3d_core::pattern::Drive::Angle { axis, .. } => {
                        let radial = simple3d_core::pattern::radial_axis(axis);
                        let zero = Vec3::axis(radial);
                        Some((along * (1.0 / scale), own.vector(zero).normalized(), grip.radius))
                    }
                    _ => None,
                };
                PatternGrip {
                    label: grip.label,
                    at: own.point(grip.at),
                    from: own.point(grip.from),
                    dir: along * (1.0 / scale),
                    scale,
                    turn,
                }
            })
            .collect()
    }

    /// The middle of what pattern `id` repeats, in the pattern's frame, from the last evaluation.
    fn pattern_content_centre(&self, id: NodeId, own: &simple3d_core::xform::Xform) -> Vec3 {
        let mut bounds: Option<(Vec3, Vec3)> = None;
        for child in &self.scene.node(id).children {
            let Some(&(lo, hi)) = self.evaluated.node_world_bounds.get(child) else { continue };
            bounds = Some(bounds.map_or((lo, hi), |(a, b)| (a.min(lo), b.max(hi))));
        }
        bounds.map_or(Vec3::ZERO, |(lo, hi)| own.inverse().point((lo + hi) * 0.5))
    }

    /// What the pointer asks a grip for: a distance along its line or a span's angle, in the
    /// pattern's own units.
    pub fn pattern_grip_value(&self, grip: &PatternGrip, view: &crate::view::View, cursor: egui::Pos2) -> Option<f64> {
        match grip.turn {
            Some((axis, zero, _)) => {
                let at = view.ray_plane(cursor, grip.from, axis)?;
                let radial = at - grip.from;
                let tangent = axis.cross(zero);
                let degrees = radial.dot(tangent).atan2(radial.dot(zero)).to_degrees();
                // Past the back of the circle a span reads as a full turn rather than zero.
                Some(if degrees < 0.0 { degrees + 360.0 } else { degrees })
            }
            None => Some(view.ray_axis(cursor, grip.from, grip.dir)? / grip.scale),
        }
    }

    /// Write a dragged grip value as one coalesced undo step, rounded to the move step or rotation
    /// snap like every other viewport drag; counts round to whole copies.
    pub fn set_pattern_grip(&mut self, id: NodeId, label: &str, value: f64, mods: gizmo::Mods) {
        let Some(params) = self.scene.get(id).and_then(|n| n.params()) else { return };
        let Some(grip) = simple3d_core::pattern::grip(params, label) else { return };
        let wanted = match grip.drive {
            simple3d_core::pattern::Drive::Length { .. } => mods.snap(value, self.move_snap()),
            simple3d_core::pattern::Drive::Angle { .. } => mods.snap(value, self.settings.rotate_snap_deg),
            simple3d_core::pattern::Drive::Count { .. } => value,
        };
        self.edit("Pattern layout", Some(&format!("pattern-grip:{id}:{label}")));
        if let Some(params) = self.scene.get_mut(id).and_then(|n| n.params_mut()) {
            simple3d_core::pattern::apply_grip(params, &grip, wanted);
        }
        self.touch();
    }

    /// Size a fresh pattern's spacing to its contents the first time it has any (issue 67), so shapes
    /// dropped into an empty pattern are not repeated at exactly their own width.
    ///
    /// Only while the numbers are still the defaults, so nothing the user set is overwritten. The
    /// children are measured, not the pattern, which would measure the repetition.
    pub(crate) fn size_fresh_patterns(&mut self) {
        let defaults = simple3d_core::pattern::default_params();
        let fresh: Vec<NodeId> = self
            .scene
            .depth_first()
            .into_iter()
            .filter(|&id| {
                let node = self.scene.node(id);
                let untouched = node.params().is_some_and(|params| {
                    let mut as_born = params.clone();
                    as_born.insert("kind".to_string(), defaults["kind"]);
                    as_born == defaults
                });
                node.is_pattern() && !node.children.is_empty() && untouched
            })
            .collect();
        for id in fresh {
            let Some(size) = self.pattern_content_size(id) else { continue };
            let kind = self.scene.node(id).params().map(|params| params["kind"]);
            if let Some(node) = self.scene.get_mut(id) {
                let mut params = simple3d_core::pattern::params_for_size(size);
                if let Some(kind) = kind {
                    params.insert("kind".to_string(), kind);
                }
                node.body = simple3d_core::scene::Body::Pattern { params };
            }
        }
    }

    /// The combined size of pattern `id`'s children, or `None` if empty. Children rather than the
    /// pattern, which would measure the repetition; everything sized to "the shape" uses this.
    pub(crate) fn pattern_content_size(&self, id: NodeId) -> Option<Vec3> {
        let mut bounds: Option<(Vec3, Vec3)> = None;
        for child in &self.scene.get(id)?.children {
            let Some((lo, hi)) = simple3d_core::eval::subtree_bounds(&self.scene, *child) else { continue };
            bounds = Some(match bounds {
                Some((l, h)) => (l.min(lo), h.max(hi)),
                None => (lo, hi),
            });
        }
        bounds.map(|(lo, hi)| hi - lo)
    }

    /// The pattern tool (issue 67): wrap the selection in a pattern, or drop an empty one at the
    /// insertion point. The pattern ends up selected.
    pub(crate) fn make_pattern(&mut self) {
        self.edit("Pattern", None);
        let created = if self.selection.is_empty() {
            let (parent, index) = self.scene.insertion_point(self.primary());
            Some(self.scene.add_pattern(parent, index))
        } else {
            // Gather the top-level selection under a new node, then make it a pattern rather than a union.
            let group = self.scene.group_selection(&self.selection.clone());
            if let Some(group) = group {
                // Measured before the node becomes a pattern, which would measure the repetition.
                let size = simple3d_core::eval::subtree_bounds(&self.scene, group)
                    .map(|(lo, hi)| hi - lo)
                    .unwrap_or(Vec3::ZERO);
                if let Some(node) = self.scene.get_mut(group) {
                    // The stock step equals the stock box's width, so copies would touch (`pattern::params_for_size`).
                    node.body =
                        simple3d_core::scene::Body::Pattern { params: simple3d_core::pattern::params_for_size(size) };
                    node.name = "Pattern".to_string();
                }
            }
            group
        };
        match created {
            Some(id) => {
                self.select_only(id);
                self.status = Status::Info("Pattern: choose its kind and numbers in the properties panel".into());
            }
            None => self.status = Status::Warning("That selection cannot be made into a pattern".into()),
        }
    }
}

/// A grip moved out to the copies it lays out. Copies are the children moved by the numbers, so
/// children off the origin repeat that much further out; carrying the grip by the children's
/// centre keeps it on its copy while reading back the same number.
fn on_the_copies(mut grip: simple3d_core::pattern::Grip, centre: Vec3) -> simple3d_core::pattern::Grip {
    match grip.drive {
        simple3d_core::pattern::Drive::Angle { .. } => {
            let axis = grip.dir.normalized();
            let lift = axis * centre.dot(axis);
            let out = (centre - lift).length();
            let reach = grip.at - grip.from;
            let rise = axis * reach.dot(axis);
            let widen = if grip.radius > 1e-9 { (grip.radius + out) / grip.radius } else { 1.0 };
            grip.from = grip.from + lift;
            grip.at = grip.from + rise + (reach - rise) * widen;
            grip.radius += out;
        }
        _ => {
            grip.at = grip.at + centre;
            grip.from = grip.from + centre;
        }
    }
    grip
}
