//! The document as a whole: whether it is edited, what it is called, and
//! the units it is read in.

use super::*;
use simple3d_core::mesh_data::MeshData;
use simple3d_core::scene::NodeId;
use simple3d_core::unit::Unit;
use std::sync::Arc;

impl App {
    // -- edits --------------------------------------------------------------

    /// Take an undo snapshot and mark the scene for re-evaluation. Every
    /// model-mutating path in the application goes through here.
    pub fn edit(&mut self, label: &str, coalesce: Option<&str>) -> bool {
        let lifted = self.lift_preview();
        let coalescing = self.history.record(&self.scene, label, coalesce);
        self.drop_preview_back(lifted);
        self.dirty = true;
        coalescing
    }

    /// Take a tool's preview out of the document for as long as it takes to
    /// snapshot it, and give back what has to go in again afterwards
    /// (issue 106).
    ///
    /// The simplify tool shows its result by putting it *in* the document,
    /// which is what makes the preview the real thing rather than a picture of
    /// it -- but it is not an edit anybody has made, and an undo step recorded
    /// over it would be a step back to a simplification nobody accepted.
    /// Renaming the mesh while the window is open, and then undoing the rename,
    /// did exactly that.
    ///
    /// Inert once the tool has let go of what it is showing, which is how the
    /// tool's own accept and cancel get their snapshots taken over the mesh
    /// they mean.
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

    /// Mark the scene for re-evaluation without taking a snapshot, for the
    /// frames *during* a drag -- the snapshot was taken when the drag began, so
    /// the whole drag is one undo step.
    pub fn touch(&mut self) {
        self.dirty = true;
    }

    pub fn unsaved(&self) -> bool {
        self.history.revision() != self.saved_revision
    }

    pub fn status_text(&self) -> String {
        self.status.text().to_string()
    }

    /// Force the viewport image to be rebuilt on the next frame.
    pub fn invalidate_image(&mut self) {
        self.image_key = u64::MAX;
    }

    pub fn unit(&self) -> Unit {
        self.scene.settings.unit
    }

    /// The move and resize snap increment, in millimetres (spec section 6.2).
    pub fn move_snap(&self) -> f64 {
        self.scene.settings.snap_step.max(1e-6)
    }

    // -- files --------------------------------------------------------------

    pub fn title(&self) -> String {
        let name = crate::tabs::document_name(self.path.as_deref());
        format!("{}{name} - {APP_NAME}", if self.unsaved() { "*" } else { "" })
    }
}
