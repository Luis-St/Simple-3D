//! The document as a whole: edited state, name, and units.

use super::*;
use simple3d_core::mesh_data::MeshData;
use simple3d_core::scene::NodeId;
use simple3d_core::unit::Unit;
use std::sync::Arc;

impl App {
    // -- edits --------------------------------------------------------------

    /// Take an undo snapshot and mark the scene dirty; every model-mutating path goes through here.
    pub fn edit(&mut self, label: &str, coalesce: Option<&str>) -> bool {
        let lifted = self.lift_preview();
        let coalescing = self.history.record(&self.scene, label, coalesce);
        self.drop_preview_back(lifted);
        self.dirty = true;
        coalescing
    }

    /// Lift a tool's preview out of the document while snapshotting (issue 106), returning what to put
    /// back. The simplify preview lives in the document but is not an edit; an undo step over it would
    /// restore an unaccepted result. Inert once the tool has let go.
    pub(crate) fn lift_preview(&mut self) -> Option<(NodeId, Arc<MeshData>)> {
        let tool = self.simplify_tool.as_ref()?;
        let showing = tool.shown.as_ref()?.mesh.clone();
        let target = tool.target;
        let original = tool.original.clone();
        self.scene.set_mesh(target, original).then_some((target, showing))
    }

    pub(crate) fn drop_preview_back(&mut self, lifted: Option<(NodeId, Arc<MeshData>)>) {
        if let Some((target, mesh)) = lifted {
            self.scene.set_mesh(target, mesh);
        }
    }

    /// Mark the scene dirty without a snapshot, for frames during a drag (one undo step overall).
    pub fn touch(&mut self) {
        self.dirty = true;
    }

    /// Whether the project has unsaved changes, in any component (issue 113).
    pub fn unsaved(&self) -> bool {
        self.history.revision() != self.saved_revision || self.project.unsaved()
    }

    pub fn status_text(&self) -> String {
        self.status.text().to_string()
    }

    /// Force the viewport image to be rebuilt next frame.
    pub fn invalidate_image(&mut self) {
        self.image_key = u64::MAX;
    }

    pub fn unit(&self) -> Unit {
        self.scene.settings.unit
    }

    /// The move and resize snap increment in millimetres (spec section 6.2).
    pub fn move_snap(&self) -> f64 {
        self.scene.settings.snap_step.max(1e-6)
    }

    // -- files --------------------------------------------------------------

    pub fn title(&self) -> String {
        let name = crate::tabs::document_name(self.path.as_deref());
        format!("{}{name} - {APP_NAME}", if self.unsaved() { "*" } else { "" })
    }
}
