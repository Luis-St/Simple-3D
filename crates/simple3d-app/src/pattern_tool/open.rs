//! Opening the tool on a selection, and the preview it starts with.

use crate::app::{App, Modal, Status};
use simple3d_core::pattern;
use simple3d_core::primitive::{ParamValue, ParamsExt};
use simple3d_geom::Vec3;

impl App {
    /// Open the tool on the selected pattern -- making one out of the selection
    /// first if what is selected is not a pattern yet, which is what makes this
    /// a *creation* tool and not merely an editor of one.
    pub fn open_pattern_tool(&mut self) {
        let existing = self.selection.iter().copied().find(|id| self.scene.get(*id).is_some_and(|n| n.is_pattern()));
        let target = match existing {
            Some(id) => Some(id),
            None => {
                self.make_pattern();
                self.selection.iter().copied().find(|id| self.scene.get(*id).is_some_and(|n| n.is_pattern()))
            }
        };
        let Some(id) = target else {
            self.status = Status::Warning("That selection cannot be made into a pattern".into());
            return;
        };
        // The tool only ever builds a custom rule, so the node is switched to
        // one on the way in. Switching kinds writes no numbers of its own --
        // every kind keeps its own -- so the six fixed kinds are still there,
        // unchanged, if the user picks one again afterwards.
        if self.scene.node(id).params().map(|p| p.int("kind")) != Some(pattern::CUSTOM) {
            self.edit("Custom pattern", None);
            if let Some(params) = self.scene.get_mut(id).and_then(|n| n.params_mut()) {
                params.insert("kind".to_string(), ParamValue::Choice(pattern::CUSTOM));
            }
        }
        self.pattern_tool = Some(id);
        self.pattern_tool_name = self.scene.node(id).name.clone();
        self.refresh_pattern_kinds();
        self.reset_pattern_preview();
        self.modal = Modal::PatternKind;
    }

    /// Put the preview back the way it opens: looking at the pattern from where
    /// the viewport is looking at the scene, backed off far enough to hold what
    /// the rule lays out.
    ///
    /// Same angle, so the picture in the window and the one behind it agree
    /// about which way round the shape is; its own camera from there, so
    /// turning one does not turn the other. The framing itself waits for the
    /// next frame, which is where the picture's own shape is known -- forgetting
    /// what it was framed on is what asks for it, in `preview`.
    ///
    /// This is what the Frame button does, and the only thing besides opening
    /// the tool that moves the preview's camera on the user's behalf. Editing
    /// the rule does not: a picture turned and zoomed to look at one end of a
    /// run has to survive the next keystroke, and Frame is how the whole of it
    /// is asked for back.
    pub(crate) fn reset_pattern_preview(&mut self) {
        self.pattern_preview_camera = self.scene.camera;
        self.pattern_preview_framed = false;
    }

    /// What the preview is looking at: the pattern, what the rule lays out while
    /// there is nothing in the pattern to lay out, or the whole scene.
    ///
    /// The middle one is the case a pattern is *built* in. A pattern with
    /// nothing in it has no bounds of its own, and framing on the scene instead
    /// pointed the picture at everything except the thing the window is open
    /// for -- and, with an empty scene behind it, at nothing at all, which is
    /// where the Frame button had nothing to do.
    pub(crate) fn pattern_preview_target(&self) -> Option<(Vec3, Vec3)> {
        self.pattern_tool
            .and_then(|id| self.evaluated.node_world_bounds.get(&id).copied())
            .or_else(|| self.pattern_placement_bounds())
            .or_else(|| self.evaluated.mesh.bounds())
    }

    /// Where the rule would put a copy, in world space. These are the
    /// placements a pattern still has when there is no shape in it to place.
    pub(crate) fn pattern_placements(&self) -> Vec<Vec3> {
        let Some(id) = self.pattern_tool_target() else { return Vec::new() };
        let Some(params) = self.scene.node(id).params() else { return Vec::new() };
        // The instances are laid out in the pattern's own frame; the gizmo's is
        // what carries that frame out into the world, the same way the lay-out
        // grips in the viewport are placed.
        let Some(gizmo) = self.gizmo_for(id) else { return Vec::new() };
        pattern::instances(params).into_iter().map(|copy| gizmo.own.point(copy.xform.t)).collect()
    }

    /// The room the placements take, with air around them -- a rule that lays
    /// out one copy is a single point, and a picture framed on a point is a
    /// grid line filling the window.
    pub(super) fn pattern_placement_bounds(&self) -> Option<(Vec3, Vec3)> {
        let places = self.pattern_placements();
        let (first, rest) = places.split_first()?;
        let (mut lo, mut hi) = (*first, *first);
        for at in rest {
            lo = lo.min(*at);
            hi = hi.max(*at);
        }
        let pad = self.scene.settings.grid_spacing.max(1.0);
        let pad = Vec3::new(pad, pad, pad);
        Some((lo - pad, hi + pad))
    }

    /// A pattern with the rule but not yet the shape it repeats. Its preview
    /// has no geometry to render, so it is drawn as its placements instead.
    pub(crate) fn pattern_tool_is_empty(&self) -> bool {
        self.pattern_tool_target().is_some_and(|id| !self.evaluated.node_world_bounds.contains_key(&id))
    }

    /// Point the preview camera at the pattern, at the aspect the picture is
    /// actually drawn at, and mark it framed.
    ///
    /// Only the target and the distance move. The angle is set where the
    /// framing is *asked* for, by `reset_pattern_preview`, because this runs a
    /// frame later -- by which time the user may already be dragging.
    pub(crate) fn frame_pattern_preview(&mut self, aspect: f64) {
        match self.pattern_preview_target() {
            Some((lo, hi)) => {
                crate::view::frame_bounds(&mut self.pattern_preview_camera, lo, hi, aspect);
                self.pattern_preview_framed = true;
            }
            // Nothing to frame on yet. The pattern's extent comes out of the
            // evaluation, which is asynchronous, so the first frames after the
            // tool opens can have no answer -- and a picture that gave up on
            // the first of them would stay pointed at the origin for good.
            // Park it and ask again next frame.
            None => {
                self.pattern_preview_camera.target = Vec3::ZERO;
                self.pattern_preview_camera.distance = 160.0;
            }
        }
    }
}
