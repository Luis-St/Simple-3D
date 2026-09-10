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
        // one on the way in -- and it starts from what the pattern was already
        // laying out rather than from the stage defaults (issue 79). Opening the
        // tool on a ring of six used to replace it with a stock run of three,
        // which threw away the layout on the way to editing it.
        //
        // Switching kinds writes no numbers of any *other* kind -- every kind
        // keeps its own -- so the six fixed kinds are still there, unchanged, if
        // the user picks one again afterwards.
        let kind = self.scene.node(id).params().map(|p| p.int("kind"));
        if kind != Some(pattern::CUSTOM) {
            self.edit("Custom pattern", None);
            if let Some(params) = self.scene.get_mut(id).and_then(|n| n.params_mut()) {
                pattern::use_as_template(params, kind.unwrap_or(0));
            }
        }
        self.pattern_tool = Some(id);
        self.pattern_tool_name = self.scene.node(id).name.clone();
        self.refresh_pattern_kinds();
    }

    /// Put the window away. Nothing is undone by it: every number the tool
    /// showed is already on the pattern, so there is nothing provisional to
    /// throw back.
    pub(crate) fn close_pattern_tool(&mut self) {
        self.pattern_tool = None;
    }

    /// Start the rule again from what one of the fixed kinds lays out
    /// (issue 79).
    pub(crate) fn start_rule_from(&mut self, id: NodeId, kind: u32) {
        if !self.scene.get(id).is_some_and(|n| n.is_pattern()) {
            return;
        }
        self.edit("Pattern rule", None);
        if let Some(params) = self.scene.get_mut(id).and_then(|n| n.params_mut()) {
            pattern::use_as_template(params, kind);
        }
        let name = pattern::KINDS.get(kind as usize).copied().unwrap_or("that kind");
        self.status = Status::Info(format!("The rule now says what {} said", name.to_lowercase()));
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

    /// A pattern with the rule but not yet the shape it repeats. There is no
    /// geometry to draw for it, so the viewport draws its placements instead.
    pub(crate) fn pattern_tool_is_empty(&self) -> bool {
        self.pattern_tool_target().is_some_and(|id| !self.evaluated.node_world_bounds.contains_key(&id))
    }
}
