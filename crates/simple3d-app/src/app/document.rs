//! The document as a whole: whether it is edited, what it is called, and
//! the units it is read in.

use super::*;
use simple3d_core::unit::Unit;

impl App {
    // -- edits --------------------------------------------------------------

    /// Take an undo snapshot and mark the scene for re-evaluation. Every
    /// model-mutating path in the application goes through here.
    pub fn edit(&mut self, label: &str, coalesce: Option<&str>) -> bool {
        let coalescing = self.history.record(&self.scene, label, coalesce);
        self.dirty = true;
        coalescing
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
