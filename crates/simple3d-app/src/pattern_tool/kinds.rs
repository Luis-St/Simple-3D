//! The saved rules on the shelf: reading, applying and writing them.

use crate::app::{App, Status};
use simple3d_core::pattern;
use simple3d_core::pattern_library;
use simple3d_core::primitive::{ParamValue, Params};
use simple3d_core::scene::NodeId;
use simple3d_geom::Vec3;

impl App {
    /// Re-read the shelf, on tool open and after writes rather than every frame.
    pub(crate) fn refresh_pattern_kinds(&mut self) {
        self.pattern_kinds = pattern_library::list(self.config_dir());
    }

    /// The pattern the tool works on, if it still exists; the non-modal tool can see it deleted.
    pub(crate) fn pattern_tool_target(&self) -> Option<NodeId> {
        self.pattern_tool.filter(|id| self.scene.get(*id).is_some_and(|n| n.is_pattern()))
    }

    pub(crate) fn pattern_tool_params(&self) -> Params {
        self.pattern_tool_target().and_then(|id| self.scene.node(id).params().cloned()).unwrap_or_default()
    }
}

impl App {
    /// Remove a stage anywhere in the stack; later ones move up and repeat what remains above.
    pub(crate) fn drop_stage(&mut self, index: usize) {
        let Some(id) = self.pattern_tool_target() else { return };
        if pattern::stage_count(&self.pattern_tool_params()) <= 1 {
            return;
        }
        self.edit("Drop pattern stage", None);
        if let Some(params) = self.scene.get_mut(id).and_then(|n| n.params_mut()) {
            pattern::remove_stage(params, index);
        }
        // Each stage's fold moves with it, not its slot.
        for below in index..pattern::MAX_STAGES - 1 {
            self.pattern_tool_folded[below] = self.pattern_tool_folded[below + 1];
        }
    }

    /// Move stage `index` one place up or down.
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
    }

    /// Add a stage doing `mode` at the end (issue 79), starting as the next thing the rule lacks
    /// (see [`pattern::fresh_stage_doing`]) rather than its slot's old contents.
    pub(crate) fn add_stage_doing(&mut self, mode: pattern::StageMode) {
        let Some(id) = self.pattern_tool_target() else { return };
        let used = pattern::stage_count(&self.pattern_tool_params());
        if used >= pattern::MAX_STAGES {
            return;
        }
        let size = self.pattern_content_size(id).unwrap_or(Vec3::ZERO);
        self.edit("Pattern stages", None);
        if let Some(params) = self.scene.get_mut(id).and_then(|n| n.params_mut()) {
            let fresh = pattern::fresh_stage_doing(params, used, size, mode);
            pattern::set_stage(params, used, &fresh);
            params.insert("stages".to_string(), ParamValue::Count(used as u32 + 1));
        }
        self.pattern_tool_folded[used] = false;
    }

    /// Add a `what` variation to stage `index` at a visible amount ([`pattern::fresh_variation`]);
    /// nothing, not even an undo step, when none is left.
    pub(crate) fn add_variation(&mut self, index: usize, what: pattern::Vary) {
        let Some(id) = self.pattern_tool_target() else { return };
        let size = self.pattern_content_size(id).unwrap_or(Vec3::ZERO);
        let Some(fresh) = pattern::fresh_variation(&self.pattern_tool_params(), index, what, size) else { return };
        self.edit("Vary pattern stage", None);
        if let Some(params) = self.scene.get_mut(id).and_then(|n| n.params_mut()) {
            pattern::add_variation(params, index, fresh);
        }
    }

    /// Remove variation `slot` from stage `index`; later ones move up.
    pub(crate) fn drop_variation(&mut self, index: usize, slot: usize) {
        let Some(id) = self.pattern_tool_target() else { return };
        if slot >= pattern::variation_count(&self.pattern_tool_params(), index) {
            return;
        }
        self.edit("Drop pattern variation", None);
        if let Some(params) = self.scene.get_mut(id).and_then(|n| n.params_mut()) {
            pattern::remove_variation(params, index, slot);
        }
    }

    /// Put a saved rule on a pattern, from the tool's start question or the property panel's shelf.
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
        // Renamed to the kind only while it still has its automatic name.
        if self.scene.node(id).name.starts_with("Pattern") {
            if let Some(node) = self.scene.get_mut(id) {
                node.name = entry.name.clone();
            }
        }
        self.pattern_tool_name = entry.name.clone();
        // On the tool's own pattern, it answers the start question.
        if self.pattern_tool_target() == Some(id) {
            self.pattern_tool_started = true;
            self.pattern_tool_resumable = false;
            self.sync_pattern_tool_sections();
        }
        let with = if pattern_library::has_noise(&kind) { ", with its noise" } else { "" };
        self.status = Status::Info(format!("Pattern kind \u{201C}{}\u{201D} applied{with}", entry.name));
    }

    /// Save the node's current rule to the shelf under the dialog's name.
    pub(crate) fn save_current_kind(&mut self) {
        let params = self.pattern_tool_params();
        let name = simple3d_core::library::sanitise(&self.pattern_tool_name);
        // The scatter is kept when asked and present (issue 79); otherwise applying leaves targets' own.
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

    /// Ask before deleting a saved kind, which undo cannot restore.
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
