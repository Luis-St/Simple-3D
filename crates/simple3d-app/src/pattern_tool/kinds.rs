//! The saved rules on the shelf: reading, applying and writing one.

use crate::app::{App, Status};
use simple3d_core::pattern;
use simple3d_core::pattern_library;
use simple3d_core::primitive::{ParamValue, Params};
use simple3d_core::scene::NodeId;
use simple3d_geom::Vec3;

impl App {
    /// Re-read the shelf. Done when the tool opens and after it is written to,
    /// rather than every frame: the shelf is a directory and the window draws
    /// sixty times a second.
    pub(crate) fn refresh_pattern_kinds(&mut self) {
        self.pattern_kinds = pattern_library::list(self.config_dir());
    }

    /// The pattern the tool is working on, if it is still there: the tool is not
    /// modal, so the outliner behind it can delete what it is working on.
    pub(crate) fn pattern_tool_target(&self) -> Option<NodeId> {
        self.pattern_tool.filter(|id| self.scene.get(*id).is_some_and(|n| n.is_pattern()))
    }

    pub(crate) fn pattern_tool_params(&self) -> Params {
        self.pattern_tool_target().and_then(|id| self.scene.node(id).params().cloned()).unwrap_or_default()
    }
}

impl App {
    /// How many stages the rule uses.
    ///
    /// A stage that is added starts as the next thing the rule is missing -- a
    /// run along an axis nothing above it runs along, spaced clear of what the
    /// pattern repeats -- rather than as whatever its slot last held (issue 79).
    /// The slot used to decide: after a grid that was stage 4's stock turn, and
    /// after a blank rule it was one copy in place, so pressing "Add a stage"
    /// appeared to do nothing at all.
    pub(crate) fn set_stage_count(&mut self, wanted: usize) {
        let Some(id) = self.pattern_tool_target() else { return };
        let wanted = wanted.clamp(1, pattern::MAX_STAGES);
        let size = self.pattern_content_size(id).unwrap_or(Vec3::ZERO);
        self.edit("Pattern stages", None);
        let mut used = wanted;
        if let Some(params) = self.scene.get_mut(id).and_then(|n| n.params_mut()) {
            used = pattern::stage_count(params);
            for index in used..wanted {
                let fresh = pattern::fresh_stage(params, index, size);
                pattern::set_stage(params, index, fresh);
            }
            params.insert("stages".to_string(), ParamValue::Count(wanted as u32));
        }
        // A stage that has just been added is the one about to be worked on.
        for index in used..wanted {
            self.pattern_tool_folded[index] = false;
            self.pattern_tool_vary_open[index] = false;
        }
    }

    /// Take one stage out of the rule, wherever it is in the stack; the ones
    /// below it move up and go on repeating what is left above them.
    pub(crate) fn drop_stage(&mut self, index: usize) {
        let Some(id) = self.pattern_tool_target() else { return };
        if pattern::stage_count(&self.pattern_tool_params()) <= 1 {
            return;
        }
        self.edit("Drop pattern stage", None);
        if let Some(params) = self.scene.get_mut(id).and_then(|n| n.params_mut()) {
            pattern::remove_stage(params, index);
        }
        // The fold of each stage goes with it, not with its slot.
        for below in index..pattern::MAX_STAGES - 1 {
            self.pattern_tool_folded[below] = self.pattern_tool_folded[below + 1];
            self.pattern_tool_vary_open[below] = self.pattern_tool_vary_open[below + 1];
        }
    }

    /// Move stage `index` one place up the stack, or down.
    pub(crate) fn move_stage(&mut self, index: usize, up: bool) {
        let Some(id) = self.pattern_tool_target() else { return };
        let used = pattern::stage_count(&self.pattern_tool_params());
        let Some(other) = (if up { index.checked_sub(1) } else { Some(index + 1) }).filter(|o| *o < used) else {
            return;
        };
        self.edit("Reorder pattern stages", None);
        if let Some(params) = self.scene.get_mut(id).and_then(|n| n.params_mut()) {
            pattern::swap_stages(params, index, other);
        }
        self.pattern_tool_folded.swap(index, other);
        self.pattern_tool_vary_open.swap(index, other);
    }

    /// Put a saved rule on a pattern: the tool's own, from the question it
    /// opens with, or any custom pattern from the property panel's shelf.
    pub(crate) fn apply_saved_kind_to(&mut self, id: NodeId, entry: &pattern_library::Entry) {
        if !self.scene.get(id).is_some_and(|n| n.is_pattern()) {
            return;
        }
        let Some(kind) = pattern_library::load(&entry.path) else {
            self.status = Status::Warning(format!("\u{201C}{}\u{201D} could not be read", entry.name));
            return;
        };
        self.edit("Pattern kind", None);
        if let Some(params) = self.scene.get_mut(id).and_then(|n| n.params_mut()) {
            pattern_library::apply(params, &kind);
        }
        // The node takes the kind's name, but only while it still carries the
        // one it was given automatically: a pattern the user has named
        // themselves keeps that name.
        if self.scene.node(id).name.starts_with("Pattern") {
            if let Some(node) = self.scene.get_mut(id) {
                node.name = entry.name.clone();
            }
        }
        self.pattern_tool_name = entry.name.clone();
        // Applied to the rule the tool is open on, it answers the question the
        // window opens with: the rule now starts from this kind.
        if self.pattern_tool_target() == Some(id) {
            self.pattern_tool_started = true;
            self.pattern_tool_resumable = false;
            self.sync_pattern_tool_sections();
        }
        let with = if pattern_library::has_noise(&kind) { ", with its noise" } else { "" };
        self.status = Status::Info(format!("Pattern kind \u{201C}{}\u{201D} applied{with}", entry.name));
    }

    /// Keep the rule the node currently holds on the shelf, under the name in
    /// the dialog's name field.
    pub(crate) fn save_current_kind(&mut self) {
        let params = self.pattern_tool_params();
        let name = simple3d_core::library::sanitise(&self.pattern_tool_name);
        // The scatter goes with the rule when the tool says to keep it and
        // there is one to keep (issue 79). A pattern with none saves none, so
        // applying the kind later leaves the other pattern's own alone.
        let with_noise =
            self.pattern_tool_keep_noise && self.pattern_tool_target().is_some_and(|id| self.noise_is_set(id));
        match pattern_library::save(self.config_dir(), &name, &params, with_noise) {
            Ok(_) => {
                self.refresh_pattern_kinds();
                let with = if with_noise { ", with its noise" } else { "" };
                self.status = Status::Info(format!("Pattern kind \u{201C}{name}\u{201D} saved{with}"));
            }
            Err(e) => self.status = Status::Warning(format!("Could not save the pattern kind: {e}")),
        }
    }

    /// Put the "delete this one" question up. Nothing is removed until it is
    /// answered: the shelf is a directory, so this is the one thing a click in
    /// the pattern tool does that undo cannot take back.
    pub(crate) fn ask_delete_saved_kind(&mut self, entry: pattern_library::Entry) {
        self.confirm_delete_kind = Some(entry);
        self.modal = crate::app::Modal::ConfirmDeleteKind;
    }

    pub(crate) fn delete_saved_kind(&mut self, entry: &pattern_library::Entry) {
        match pattern_library::remove(&entry.path) {
            Ok(()) => {
                self.refresh_pattern_kinds();
                self.status = Status::Info(format!("Pattern kind \u{201C}{}\u{201D} deleted", entry.name));
            }
            Err(e) => self.status = Status::Warning(format!("Could not delete the pattern kind: {e}")),
        }
    }
}
