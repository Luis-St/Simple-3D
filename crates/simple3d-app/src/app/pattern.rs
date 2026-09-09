//! Patterns: their handles, and making one from a selection.

use super::*;
use crate::gizmo::{self};
use simple3d_core::scene::NodeId;
use simple3d_geom::Vec3;

impl App {
    /// Every lay-out grip the pattern `id` offers, in world space (issue 67).
    ///
    /// Empty for anything that is not a pattern, and for a mirror, which has no
    /// distance and no count to lay out. The pattern's own frame is what places
    /// them: a grip marks a copy the pattern actually makes, and those turn
    /// with the node.
    pub fn pattern_grips(&self, id: NodeId) -> Vec<PatternGrip> {
        let Some(node) = self.scene.get(id) else { return Vec::new() };
        if !node.is_pattern() {
            return Vec::new();
        }
        let Some(params) = node.params() else { return Vec::new() };
        let Some(gizmo) = self.gizmo_for(id) else { return Vec::new() };
        let own = gizmo.own;
        simple3d_core::pattern::grips(params)
            .into_iter()
            .map(|grip| {
                let along = own.vector(grip.dir);
                let scale = along.length().max(1e-9);
                let turn = match grip.drive {
                    simple3d_core::pattern::Drive::Angle { axis, .. } => {
                        let radial = simple3d_core::pattern::radial_axis(axis);
                        let mut zero = Vec3::ZERO;
                        match radial {
                            0 => zero.x = 1.0,
                            1 => zero.y = 1.0,
                            _ => zero.z = 1.0,
                        }
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

    /// What the pointer is asking a grip for: a distance along its line, or --
    /// for a span grip -- the angle it has been carried round to.
    ///
    /// Both come back in the pattern's own units, which is what its parameters
    /// are written in, so a scaled pattern is read back at its own numbers
    /// rather than the world's.
    pub fn pattern_grip_value(&self, grip: &PatternGrip, view: &crate::view::View, cursor: egui::Pos2) -> Option<f64> {
        match grip.turn {
            Some((axis, zero, _)) => {
                let at = view.ray_plane(cursor, grip.from, axis)?;
                let radial = at - grip.from;
                let tangent = axis.cross(zero);
                let degrees = radial.dot(tangent).atan2(radial.dot(zero)).to_degrees();
                // Round the back of the circle a span reads as the whole turn
                // rather than as nothing: dragging past 359 degrees means "all
                // the way", which is the number a full ring wants.
                Some(if degrees < 0.0 { degrees + 360.0 } else { degrees })
            }
            None => Some(view.ray_axis(cursor, grip.from, grip.dir)? / grip.scale),
        }
    }

    /// Write what a grip was dragged to, coalesced into one undo step so the
    /// whole drag is a single edit.
    ///
    /// The value is rounded the way every other viewport drag is -- to the
    /// document's move step for a distance, to the rotation snap for a span --
    /// because a handle that alone produced "7.0359 mm" under a 1 mm step was
    /// the odd one out. A count rounds to whole copies on its own.
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

    /// Size a fresh pattern's spacing to what it holds, the moment it first
    /// holds something (issue 67).
    ///
    /// The pattern *tool* measures the shapes it wraps, so making a pattern of a
    /// 20 mm box gives a 30 mm step. A pattern made with nothing selected has
    /// nothing to measure yet and keeps the stock numbers, so a 20 mm shape
    /// dropped into it afterwards was repeated at exactly its own width and the
    /// copies came out as one welded bar instead of three boxes standing clear.
    ///
    /// Only while the numbers are still untouched -- exactly the defaults a bare
    /// pattern is born with -- so nothing typed into the property editor, and
    /// nothing laid out with a grip, is ever overwritten under the user.
    ///
    /// The children are measured, not the pattern: asking the pattern measures
    /// the repetition rather than the thing being repeated, and the spacing
    /// derived from that comes out a whole run too large.
    pub(crate) fn size_fresh_patterns(&mut self) {
        let defaults = simple3d_core::pattern::default_params();
        let fresh: Vec<NodeId> = self
            .scene
            .depth_first()
            .into_iter()
            .filter(|&id| {
                let node = self.scene.node(id);
                node.is_pattern() && !node.children.is_empty() && node.params() == Some(&defaults)
            })
            .collect();
        for id in fresh {
            let mut bounds: Option<(Vec3, Vec3)> = None;
            for child in self.scene.node(id).children.clone() {
                let Some((lo, hi)) = simple3d_core::eval::subtree_bounds(&self.scene, child) else { continue };
                bounds = Some(match bounds {
                    Some((l, h)) => (l.min(lo), h.max(hi)),
                    None => (lo, hi),
                });
            }
            let Some((lo, hi)) = bounds else { continue };
            if let Some(node) = self.scene.get_mut(id) {
                node.body =
                    simple3d_core::scene::Body::Pattern { params: simple3d_core::pattern::params_for_size(hi - lo) };
            }
        }
    }

    /// The pattern creation tool (issue 67): wrap the selection in a pattern
    /// node that repeats it, or -- with nothing selected -- drop an empty pattern
    /// at the insertion point for shapes to be put under. Either way the pattern
    /// is selected, so the property editor is right there to lay it out.
    pub(crate) fn make_pattern(&mut self) {
        self.edit("Pattern", None);
        let created = if self.selection.is_empty() {
            let (parent, index) = self.scene.insertion_point(self.primary());
            Some(self.scene.add_pattern(parent, index))
        } else {
            // Reuse the grouping logic to gather the top-level selection under one
            // new node, then make that node a pattern rather than a union.
            let group = self.scene.group_selection(&self.selection.clone());
            if let Some(group) = group {
                // Measured *before* the node becomes a pattern. Asking afterwards
                // measures the repetition rather than the thing being repeated --
                // three 20 mm boxes 20 mm apart read as 60 mm wide, and the
                // spacing derived from that came out three times too large.
                let size = simple3d_core::eval::subtree_bounds(&self.scene, group)
                    .map(|(lo, hi)| hi - lo)
                    .unwrap_or(Vec3::ZERO);
                if let Some(node) = self.scene.get_mut(group) {
                    // The stock 20 mm step is exactly the width of the stock box,
                    // so a pattern made from one laid its copies down touching --
                    // see `pattern::params_for_size`.
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
