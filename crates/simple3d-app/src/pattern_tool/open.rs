//! Opening the tool on a selection, and closing it.

use crate::app::{App, Status};
use simple3d_core::pattern;
use simple3d_core::primitive::ParamsExt;
use simple3d_core::scene::NodeId;
use simple3d_geom::Vec3;

impl App {
    /// Open the tool on the selected pattern, first making one from the selection if needed.
    pub fn open_pattern_tool(&mut self) {
        self.show_pattern_tool(None);
    }

    /// The same, on a pattern just added for the tool by the outliner's Add menu.
    pub(crate) fn open_pattern_tool_on_new(&mut self, id: NodeId) {
        self.show_pattern_tool(Some(id));
    }

    /// `made_for_it` is the pattern created by the action opening the tool, if any.
    fn show_pattern_tool(&mut self, made_for_it: Option<NodeId>) {
        let existing = self.selection.iter().copied().find(|id| self.scene.get(*id).is_some_and(|n| n.is_pattern()));
        let (target, mut made) = match existing {
            Some(id) => (Some(id), made_for_it == Some(id)),
            None => {
                self.make_pattern();
                (self.selection.iter().copied().find(|id| self.scene.get(*id).is_some_and(|n| n.is_pattern())), true)
            }
        };
        let Some(id) = target else {
            self.status = Status::Warning("That selection cannot be made into a pattern".into());
            return;
        };
        made &= self.scene.get(id).is_some_and(|n| n.is_pattern());
        // A pattern made by this action is custom and blank from the start (one copy, all zeros), so
        // neither the Kind row nor a default run contradicts what was asked for.
        if made {
            if let Some(params) = self.scene.get_mut(id).and_then(|n| n.params_mut()) {
                pattern::clear_stages(params);
            }
            // Point to the tool's window, where the next choice is made.
            self.status = Status::Info("Custom pattern: pick what its rule starts from".into());
        }
        // A custom pattern opens on its rule. Anything else first asks what to start from (issue 79),
        // writing nothing until answered; a pattern just made for the tool also still asks.
        let kind = self.scene.node(id).params().map(|p| p.int("kind"));
        self.pattern_tool = Some(id);
        self.pattern_tool_started = !made && kind == Some(pattern::CUSTOM);
        self.pattern_tool_resumable = false;
        self.pattern_tool_name = self.scene.node(id).name.clone();
        self.sync_pattern_tool_sections();
        self.refresh_pattern_kinds();
    }

    /// Unfold every stage when a whole rule arrives, so nothing new is hidden.
    pub(crate) fn sync_pattern_tool_sections(&mut self) {
        self.pattern_tool_folded = [false; pattern::MAX_STAGES];
    }

    /// Bring the start question back over the rule (issue 79); nothing is written until answered.
    pub(crate) fn start_rule_over(&mut self) {
        self.pattern_tool_started = false;
        self.pattern_tool_resumable = true;
        self.pattern_tool_hover = None;
    }

    /// Leave the question unanswered, back to the rule it was asked over.
    pub(crate) fn resume_rule(&mut self) {
        self.pattern_tool_started = true;
        self.pattern_tool_resumable = false;
    }

    /// Start the rule from a ready-made layout, sized to the repeated shape (issue 79).
    pub(crate) fn start_rule_from_preset(&mut self, id: NodeId, preset: usize) {
        if !self.scene.get(id).is_some_and(|n| n.is_pattern()) {
            return;
        }
        let size = self.pattern_content_size(id).unwrap_or(Vec3::ZERO);
        self.edit("Pattern rule", None);
        if let Some(params) = self.scene.get_mut(id).and_then(|n| n.params_mut()) {
            pattern::use_preset(params, preset, size);
        }
        self.pattern_tool_started = true;
        self.pattern_tool_resumable = false;
        self.sync_pattern_tool_sections();
        let name = pattern::PRESETS.get(preset).copied().unwrap_or("that layout");
        self.status = Status::Info(format!("The rule now lays out {}", name.to_lowercase()));
    }

    /// Close the window; nothing is undone, since every number is already on the pattern.
    pub(crate) fn close_pattern_tool(&mut self) {
        self.pattern_tool = None;
        self.pattern_tool_hover = None;
    }

    /// Start the rule from what a fixed kind lays out, or from nothing for [`pattern::CUSTOM`]
    /// (issue 79), using the kind's current numbers rather than its defaults.
    pub(crate) fn start_rule_from(&mut self, id: NodeId, kind: u32) {
        if !self.scene.get(id).is_some_and(|n| n.is_pattern()) {
            return;
        }
        self.edit("Pattern rule", None);
        if let Some(params) = self.scene.get_mut(id).and_then(|n| n.params_mut()) {
            if kind == pattern::CUSTOM {
                pattern::clear_stages(params);
            } else {
                pattern::use_as_template(params, kind);
            }
        }
        self.pattern_tool_started = true;
        self.pattern_tool_resumable = false;
        self.sync_pattern_tool_sections();
        self.status = Status::Info(match kind {
            pattern::CUSTOM => "The rule starts empty".to_string(),
            _ => {
                let name = pattern::KINDS.get(kind as usize).copied().unwrap_or("that kind");
                format!("The rule now says what {} said", name.to_lowercase())
            }
        });
    }

    /// Where the rule would put copies, in world space, marked so a rule can be built before it
    /// has anything to repeat.
    pub(crate) fn pattern_placements(&self) -> Vec<Vec3> {
        let Some(id) = self.pattern_tool_target() else { return Vec::new() };
        let Some(params) = self.scene.node(id).params() else { return Vec::new() };
        // Instances are in the pattern's frame; the gizmo's frame carries them into the world, as for grips.
        let Some(gizmo) = self.gizmo_for(id) else { return Vec::new() };
        pattern::instances(params).into_iter().map(|copy| gizmo.own.point(copy.xform.t)).collect()
    }

    /// Where the rule has put copies by the end of stage `last`, before scatter, in world space
    /// (issue 79).
    pub(crate) fn pattern_placements_through(&self, last: usize) -> Vec<Vec3> {
        let Some(id) = self.pattern_tool_target() else { return Vec::new() };
        let Some(params) = self.scene.node(id).params() else { return Vec::new() };
        let Some(gizmo) = self.gizmo_for(id) else { return Vec::new() };
        pattern::instances_through(params, last).into_iter().map(|copy| gizmo.own.point(copy.xform.t)).collect()
    }

    /// A pattern with a rule but nothing to repeat yet; the viewport draws its placements instead.
    pub(crate) fn pattern_tool_is_empty(&self) -> bool {
        self.pattern_tool_target().is_some_and(|id| !self.evaluated.node_world_bounds.contains_key(&id))
    }
}
