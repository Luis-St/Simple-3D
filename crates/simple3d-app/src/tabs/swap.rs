//! Putting the on-screen document away and bringing another out.

use super::*;
use crate::app::App;

impl App {
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// The tab's name and unsaved flag; the active tab is answered from `App`, since its entry is a stand-in.
    pub fn tab_summary(&self, index: usize) -> (String, bool) {
        if index == self.active {
            (document_name(self.path.as_deref()), self.unsaved())
        } else {
            let doc = &self.tabs[index];
            (doc.name(), doc.unsaved())
        }
    }

    /// True when any open document has unsaved changes, as quitting must ask.
    pub fn any_unsaved(&self) -> bool {
        self.unsaved() || self.tabs.iter().enumerate().any(|(i, doc)| i != self.active && doc.unsaved())
    }

    /// Lift the current document off `App`, clearing document fields and leaving window ones.
    pub(super) fn detach(&mut self) -> Document {
        // Undo the simplify preview first, or the tab would return holding an unaccepted result (issue 106).
        self.cancel_simplify_tool();
        let model = self.take_model();
        Document { model, path: self.path.take(), project: std::mem::take(&mut self.project) }
    }

    /// Make `doc` the document on screen, dropping half-done interactions that belonged to the previous one.
    pub(super) fn attach(&mut self, doc: Document) {
        self.put_model(doc.model);
        self.path = doc.path;
        self.project = doc.project;
        self.relink_components();
        self.after_switch();
    }

    /// Clear everything tied to the previous model, after switching document or component (issue 113).
    pub(crate) fn after_switch(&mut self) {
        self.drag = None;
        self.grabbed = None;
        self.hover_handle = None;
        self.rename = None;
        self.outliner_drag = None;
        self.drop_target = None;
        self.outliner_last_click = None;
        self.pending_delete = None;
        self.camera_move = None;
        self.cube_spin = None;
        // The measure tool is put away too, or its crosshair would eat the first click in the new document.
        self.measure = crate::app::Measure::default();
        self.fields.clear();
        self.export_preview = Default::default();
        // A split in flight is for the other document and could never apply here, so stop it.
        if let Some(job) = self.split_job.take() {
            job.cancel();
        }
        // An import in flight belongs to the other document, so stop it.
        if let Some(job) = self.import_job.take() {
            job.cancel();
        }
        self.split_tool = None;
        self.simplify_tool = None;
        self.reassemble_tool = None;

        // Nothing cached about the previous model survives.
        self.evaluation_generation += 1;
        self.scene_renderable = crate::render::Renderable::prepare_scene(&self.evaluated.mesh, &self.evaluated.ranges);
        self.node_renderables.clear();
        self.settling = None;
        self.renderable_key = u64::MAX;
        self.invalidate_image();

        // Submitted now as a supersede, or the left tab's evaluation would land on this one.
        self.worker.supersede(&self.scene);
        self.dirty = false;
    }
}
