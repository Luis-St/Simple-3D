//! The saved rules on the shelf: reading, applying and writing one.

use crate::app::{App, Status};
use simple3d_core::pattern;
use simple3d_core::pattern_library;
use simple3d_core::primitive::{ParamValue, Params};
use simple3d_core::scene::NodeId;

impl App {
    /// Re-read the shelf. Done when the tool opens and after it is written to,
    /// rather than every frame: the shelf is a directory and the dialog draws
    /// sixty times a second.
    pub(crate) fn refresh_pattern_kinds(&mut self) {
        self.pattern_kinds = pattern_library::list(self.config_dir());
    }

    /// The pattern the tool is working on, if it is still there: the outliner
    /// behind the dialog can delete it while the dialog is open.
    pub(super) fn pattern_tool_target(&self) -> Option<NodeId> {
        self.pattern_tool.filter(|id| self.scene.get(*id).is_some_and(|n| n.is_pattern()))
    }

    pub(super) fn pattern_tool_params(&self) -> Params {
        self.pattern_tool_target().and_then(|id| self.scene.node(id).params().cloned()).unwrap_or_default()
    }
}

impl App {
    /// How many stages the rule uses. Adding one starts it from the defaults it
    /// was born with, so a stage that has just appeared does something visible
    /// rather than sitting at zero and looking broken.
    pub(super) fn set_stage_count(&mut self, wanted: usize) {
        let Some(id) = self.pattern_tool_target() else { return };
        let wanted = wanted.clamp(1, pattern::MAX_STAGES) as u32;
        self.edit("Pattern stages", None);
        if let Some(params) = self.scene.get_mut(id).and_then(|n| n.params_mut()) {
            params.insert("stages".to_string(), ParamValue::Count(wanted));
        }
    }

    /// Put a saved rule on the node the tool is working on.
    pub(crate) fn apply_saved_kind(&mut self, entry: &pattern_library::Entry) {
        let Some(id) = self.pattern_tool_target() else { return };
        self.apply_saved_kind_to(id, entry);
    }

    /// The same, on a named pattern rather than the tool's own: the property
    /// panel offers the shelf on any custom pattern, with no tool open.
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
        self.status = Status::Info(format!("Pattern kind \u{201C}{}\u{201D} applied", entry.name));
    }

    /// Keep the rule the node currently holds on the shelf, under the name in
    /// the dialog's name field.
    pub(crate) fn save_current_kind(&mut self) {
        let params = self.pattern_tool_params();
        let name = simple3d_core::library::sanitise(&self.pattern_tool_name);
        match pattern_library::save(self.config_dir(), &name, &params) {
            Ok(_) => {
                self.refresh_pattern_kinds();
                self.status = Status::Info(format!("Pattern kind \u{201C}{name}\u{201D} saved"));
            }
            Err(e) => self.status = Status::Warning(format!("Could not save the pattern kind: {e}")),
        }
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
