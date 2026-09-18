//! Opening the tool on a mesh.

use super::*;
use crate::app::{App, Status};
use simple3d_core::xform::Xform;

impl App {
    /// Open the tool on the selection (issue 108).
    ///
    /// Only on a mesh, and only on one at a time. Everything else in the tree
    /// is already the thing this makes: a group is objects, a primitive is a
    /// shape with its parameters in front of you, and there is nothing in
    /// either of them to recover.
    pub fn open_reassemble_tool(&mut self) {
        let targets = self.top_level_selection();
        let Some(&id) = targets.first() else {
            self.status = Status::Warning("Select a mesh to reassemble".into());
            return;
        };
        if targets.len() > 1 {
            self.status = Status::Warning("Reassemble one mesh at a time".into());
            return;
        }
        let Some(mesh) = self.scene.get(id).and_then(|node| node.mesh()).cloned() else {
            self.status =
                Status::Warning("Only a mesh can be reassembled -- everything else is objects already".into());
            return;
        };
        if mesh.triangle_count() == 0 {
            self.status = Status::Warning("There is no geometry there to take apart".into());
            return;
        }
        self.reassemble_tool = Some(ReassembleTool {
            target: id,
            mesh,
            plan: self.settings.last_reassemble,
            found: None,
            job: None,
            outlines: true,
            placement: self.reassemble_placement(id),
            generation: self.evaluation_generation,
        });
    }

    /// Where the mesh's own frame stands in the world: the node's transform
    /// with the anchor shift on top, which is the composition the evaluation
    /// itself places the node with -- so what the preview draws lands exactly on
    /// the surface rather than an anchor's distance off it.
    pub(super) fn reassemble_placement(&self, id: NodeId) -> Xform {
        let parent = self.evaluated.node_frames.get(&id).copied().unwrap_or(Xform::IDENTITY);
        simple3d_core::eval::baked_mesh_in_place(&self.scene, id, parent).1
    }
}
