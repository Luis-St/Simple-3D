//! Opening the tool on a mesh.

use super::*;
use crate::app::{App, Status};
use simple3d_core::xform::Xform;

impl App {
    /// Open the tool on the selection (issue 108): one mesh only, since groups and primitives have
    /// nothing to recover.
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

    /// The mesh frame in the world: node transform plus anchor shift, as evaluation places it, so the
    /// preview lands on the surface.
    pub(super) fn reassemble_placement(&self, id: NodeId) -> Xform {
        let parent = self.evaluated.node_frames.get(&id).copied().unwrap_or(Xform::IDENTITY);
        simple3d_core::eval::baked_mesh_in_place(&self.scene, id, parent).1
    }
}
