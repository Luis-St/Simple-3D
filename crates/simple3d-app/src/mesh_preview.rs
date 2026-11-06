//! A mesh tool's result standing in the document in place of the mesh (issues 81 and 106).
//!
//! Simplify and the mesh functions both judge their result by looking at it, so the real surface goes
//! into the document while their window is open, without an undo step; Apply keeps it and Cancel
//! puts the original back. At most one of them is open at a time, so everything that has to tell
//! the preview from what is really there asks here: undo snapshots and saves lift it out
//! (`App::lift_preview`), and the panels and status bar read the model as committed.

mod committed;
pub(crate) use committed::*;

use crate::app::App;
use simple3d_core::mesh_data::MeshData;
use simple3d_core::scene::NodeId;
use std::sync::Arc;

/// A result standing in the document.
pub(crate) struct Standing<'a> {
    pub target: NodeId,
    /// The mesh the node really holds, put back for snapshots and on Cancel.
    pub original: &'a Arc<MeshData>,
    /// The result in its place.
    pub showing: &'a Arc<MeshData>,
}

impl App {
    /// The open mesh tool's target and the original it keeps, previewing or not.
    pub(crate) fn mesh_tool_original(&self) -> Option<(NodeId, &Arc<MeshData>)> {
        if let Some(tool) = &self.simplify_tool {
            return Some((tool.target, &tool.original));
        }
        self.shape_tool.as_ref().map(|tool| (tool.target, &tool.original))
    }

    /// The result standing in the document, once a run has landed.
    pub(crate) fn standing_preview(&self) -> Option<Standing<'_>> {
        if let Some(tool) = &self.simplify_tool {
            let showing = &tool.shown.as_ref()?.mesh;
            return Some(Standing { target: tool.target, original: &tool.original, showing });
        }
        let tool = self.shape_tool.as_ref()?;
        let showing = &tool.shown.as_ref()?.mesh;
        Some(Standing { target: tool.target, original: &tool.original, showing })
    }

    /// Close whichever mesh tool is open, putting its mesh back: the two share the one preview.
    pub(crate) fn cancel_mesh_tools(&mut self) {
        self.cancel_simplify_tool();
        self.cancel_shape_tool();
    }
}
