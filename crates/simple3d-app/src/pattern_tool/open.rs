//! Opening the tool on a selection, and closing it again.

use crate::app::{App, Status};
use simple3d_core::pattern;
use simple3d_core::primitive::ParamsExt;
use simple3d_core::scene::NodeId;
use simple3d_geom::Vec3;

impl App {
    /// Open the tool on the selected pattern -- making one out of the selection
    /// first if what is selected is not a pattern yet, which is what makes this
    /// a *creation* tool and not merely an editor of one.
    pub fn open_pattern_tool(&mut self) {
        self.show_pattern_tool(None);
    }

    /// The same, on a pattern that was added for the tool a moment ago: the
    /// outliner's Add menu puts an empty pattern on a row and then opens the
    /// tool on it, which is the custom-kind route just as much as the one that
    /// makes the node here.
    pub(crate) fn open_pattern_tool_on_new(&mut self, id: NodeId) {
        self.show_pattern_tool(Some(id));
    }

    /// `made_for_it` is the pattern that was created by the very action that is
    /// opening the tool, where there was one.
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
        // A pattern made *by* this action is a custom one from the moment it
        // exists, and it starts blank: one copy, every number zero. The menu
        // item that made it says custom, so a Kind row reading "Linear" behind
        // the tool that is about to build a rule out of stages contradicts the
        // thing the user just asked for -- and so does a run of three copies at
        // a 20 mm step, which is the linear kind's layout wearing the custom
        // kind's name. A rule built by hand starts from nothing and has each
        // stage put on it; the six fixed layouts are what the question below
        // offers for anyone who would rather not start there.
        if made {
            if let Some(params) = self.scene.get_mut(id).and_then(|n| n.params_mut()) {
                pattern::clear_stages(params);
            }
            // What made the node said to choose a kind in the properties panel,
            // which is now both answered and in the wrong place: the window in
            // front of it is what the next choice is made in.
            self.status = Status::Info("Custom pattern: pick what its rule starts from".into());
        }
        // A pattern that is already custom has a rule, so the window opens on
        // it. Anything else has a *layout* but not yet a rule, and the first
        // thing the window asks is what to start that rule from (issue 79):
        // one of the six fixed kinds -- which are laid out as stages that say
        // exactly what the kind says, so nothing is thrown away -- or nothing
        // at all. Nothing is written to the node until that is answered, so a
        // window opened by accident and closed again changes no numbers.
        //
        // One that was made for the tool is the exception: its kind was written
        // a moment ago, and it has no rule yet, so the question is still open.
        let kind = self.scene.node(id).params().map(|p| p.int("kind"));
        self.pattern_tool = Some(id);
        self.pattern_tool_started = !made && kind == Some(pattern::CUSTOM);
        self.pattern_tool_resumable = false;
        self.pattern_tool_name = self.scene.node(id).name.clone();
        self.sync_pattern_tool_sections();
        self.refresh_pattern_kinds();
    }

    /// Unfold every stage, and open the "Vary" section of each one that varies
    /// anything -- done whenever a whole rule arrives at once, so nothing a
    /// stage does is folded out of sight of someone who has not seen it yet.
    pub(crate) fn sync_pattern_tool_sections(&mut self) {
        let params = self.pattern_tool_params();
        for index in 0..pattern::MAX_STAGES {
            self.pattern_tool_folded[index] = false;
            self.pattern_tool_vary_open[index] = pattern::stage(&params, index).varies();
        }
    }

    /// Put the start question back up over the rule (issue 79).
    ///
    /// Nothing is written until it is answered, exactly as when the window
    /// first opens: the rule stays on the pattern, and the question offers the
    /// way back to it as well as the ways to replace it.
    pub(crate) fn start_rule_over(&mut self) {
        self.pattern_tool_started = false;
        self.pattern_tool_resumable = true;
        self.pattern_tool_hover = None;
    }

    /// Leave the question without answering it, back to the rule it was asked
    /// over.
    pub(crate) fn resume_rule(&mut self) {
        self.pattern_tool_started = true;
        self.pattern_tool_resumable = false;
    }

    /// Start the rule from one of the layouts that ship ready made, sized to
    /// what the pattern repeats (issue 79).
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

    /// Put the window away. Nothing is undone by it: every number the tool
    /// showed is already on the pattern, so there is nothing provisional to
    /// throw back.
    pub(crate) fn close_pattern_tool(&mut self) {
        self.pattern_tool = None;
        self.pattern_tool_hover = None;
    }

    /// Start the rule from what one of the fixed kinds lays out, or -- for
    /// [`pattern::CUSTOM`] -- from nothing at all (issue 79).
    ///
    /// The numbers come from the kind as the pattern is currently holding it,
    /// not from the kind's defaults: a ring laid out by eye and then opened in
    /// the tool starts as *that* ring.
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

    /// Where the rule would put a copy, in world space. These are the
    /// placements a pattern still has when there is no shape in it to place,
    /// and the viewport marks them so a rule can be built before there is
    /// anything for it to repeat.
    pub(crate) fn pattern_placements(&self) -> Vec<Vec3> {
        let Some(id) = self.pattern_tool_target() else { return Vec::new() };
        let Some(params) = self.scene.node(id).params() else { return Vec::new() };
        // The instances are laid out in the pattern's own frame; the gizmo's is
        // what carries that frame out into the world, the same way the lay-out
        // grips in the viewport are placed.
        let Some(gizmo) = self.gizmo_for(id) else { return Vec::new() };
        pattern::instances(params).into_iter().map(|copy| gizmo.own.point(copy.xform.t)).collect()
    }

    /// Where the rule has put its copies by the end of stage `last`, in world
    /// space and before any scatter: what the viewport marks while the pointer
    /// is over that stage in the tool (issue 79).
    pub(crate) fn pattern_placements_through(&self, last: usize) -> Vec<Vec3> {
        let Some(id) = self.pattern_tool_target() else { return Vec::new() };
        let Some(params) = self.scene.node(id).params() else { return Vec::new() };
        let Some(gizmo) = self.gizmo_for(id) else { return Vec::new() };
        pattern::instances_through(params, last).into_iter().map(|copy| gizmo.own.point(copy.xform.t)).collect()
    }

    /// A pattern with the rule but not yet the shape it repeats. There is no
    /// geometry to draw for it, so the viewport draws its placements instead.
    pub(crate) fn pattern_tool_is_empty(&self) -> bool {
        self.pattern_tool_target().is_some_and(|id| !self.evaluated.node_world_bounds.contains_key(&id))
    }
}
