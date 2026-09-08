//! Putting the document on screen away and bringing another out.

use super::*;
use crate::app::App;
use simple3d_core::scene::Scene;
use simple3d_core::undo::History;

impl App {
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// What the tab at `index` is called, and whether it has unsaved changes.
    /// The active tab is answered from the live state on `App`, since the entry
    /// in `tabs` is only a stand-in.
    pub fn tab_summary(&self, index: usize) -> (String, bool) {
        if index == self.active {
            (document_name(self.path.as_deref()), self.unsaved())
        } else {
            let doc = &self.tabs[index];
            (doc.name(), doc.unsaved())
        }
    }

    /// True when *any* open document has unsaved changes -- the question quit
    /// has to ask, rather than only about the one on screen.
    pub fn any_unsaved(&self) -> bool {
        self.unsaved() || self.tabs.iter().enumerate().any(|(i, doc)| i != self.active && doc.unsaved())
    }

    /// Lift the current document off `App`, leaving the fields that belong to a
    /// document empty and the ones that belong to the window alone.
    pub(super) fn detach(&mut self) -> Document {
        Document {
            scene: std::mem::replace(&mut self.scene, Scene::new()),
            history: std::mem::replace(&mut self.history, History::new()),
            path: self.path.take(),
            saved_revision: self.saved_revision,
            selection: std::mem::take(&mut self.selection),
            selection_anchor: self.selection_anchor.take(),
            collapsed: std::mem::take(&mut self.collapsed),
            cursor: self.cursor.take(),
            frame_when_evaluated: self.frame_when_evaluated,
            evaluated: std::mem::replace(&mut self.evaluated, empty_evaluation()),
        }
    }

    /// Make `doc` the document the application is showing.
    ///
    /// Everything half-done belongs to the document that was on screen -- a
    /// drag, a rename, a deletion waiting to be confirmed, a half-typed field --
    /// so all of it is dropped rather than carried onto a model it was never
    /// about.
    pub(super) fn attach(&mut self, doc: Document) {
        self.scene = doc.scene;
        self.history = doc.history;
        self.path = doc.path;
        self.saved_revision = doc.saved_revision;
        self.selection = doc.selection;
        self.selection_anchor = doc.selection_anchor;
        self.collapsed = doc.collapsed;
        self.cursor = doc.cursor;
        self.frame_when_evaluated = doc.frame_when_evaluated;
        self.evaluated = doc.evaluated;

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
        // A measurement is about the model that was on screen, so it does not
        // travel to the next one -- and neither does the tool holding the
        // pointer. Clearing only the span left the crosshair armed over a
        // document the user had just switched to, ready to eat their first
        // click, which is not what putting the tool away means.
        self.measure = crate::app::Measure::default();
        self.fields.clear();
        self.export_preview = None;
        // A split in flight is cutting the *other* document's shape and could
        // never be applied to this one -- `poll_split` would refuse it on the
        // tab it was started in -- so it is stopped here rather than left to
        // finish work nothing will use. The window that starts one is modal, so
        // there is never a tool open to put away as well.
        if let Some(job) = self.split_job.take() {
            job.cancel();
        }
        self.split_tool = None;

        // Nothing cached about the model on screen survives a change of model.
        self.evaluation_generation += 1;
        self.scene_renderable = crate::render::Renderable::prepare(&self.evaluated.mesh);
        self.node_renderables.clear();
        self.renderable_key = u64::MAX;
        self.invalidate_image();

        // Submit here rather than leaving `dirty` for the next frame, and as a
        // supersede rather than an ordinary edit: an evaluation of the tab we
        // just left would otherwise come back and be applied to this one.
        self.worker.supersede(&self.scene);
        self.dirty = false;
    }
}
