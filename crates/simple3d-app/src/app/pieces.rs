//! The pieces a split produced: holding them, ticking them, and previewing one.

use super::*;
use simple3d_core::scene::NodeId;

impl App {
    /// Bake the selection into stored geometry (issue 80), in each node's own frame so nothing moves.
    pub fn convert_selection_to_mesh(&mut self) {
        let targets = self.top_level_selection();
        if targets.is_empty() {
            self.status = Status::Warning("Select something to convert".into());
            return;
        }
        if targets.iter().all(|&id| self.scene.node(id).is_mesh()) {
            self.status = Status::Info("That is already a mesh".into());
            return;
        }
        let mut baked: Vec<(NodeId, simple3d_core::mesh_data::MeshData)> = Vec::new();
        for &id in &targets {
            if self.scene.node(id).is_mesh() {
                continue;
            }
            let mesh = simple3d_core::eval::baked_mesh(&self.scene, id);
            if mesh.triangle_count() == 0 {
                continue;
            }
            baked.push((id, simple3d_core::mesh_data::MeshData::new(mesh)));
        }
        if baked.is_empty() {
            self.status = Status::Warning("There is no geometry there to convert".into());
            return;
        }
        self.edit("Convert to a mesh", None);
        let mut triangles = 0;
        for (id, mesh) in baked {
            triangles += mesh.triangle_count();
            self.scene.convert_to_mesh(id, mesh);
        }
        // The children went with the old body, so selections inside them are gone.
        self.selection.retain(|id| self.scene.contains(*id));
        if self.selection.is_empty() {
            self.select_only(targets[0]);
        }
        self.status = Status::Info(format!(
            "Converted to {triangles} triangle{} of geometry -- the parameters behind it are gone",
            plural(triangles)
        ));
    }

    /// Stand a split holding `pieces` where a node stands, and select it (issue 82). An existing split
    /// keeps its identity and original, only replacing pieces, so one Join undoes any re-cut. Returns
    /// the source name and piece count, or `None` if nothing changed (the caller drops its snapshot).
    pub(crate) fn hold_pieces(
        &mut self,
        id: NodeId,
        pieces: Vec<simple3d_geom::Mesh>,
        plan: Option<simple3d_geom::tiling::SplitPlan>,
    ) -> Option<(String, usize)> {
        let count = pieces.len();
        let split = if self.scene.node(id).is_split() {
            for child in self.scene.node(id).children.clone() {
                self.scene.remove(child);
            }
            if let Some(node) = self.scene.get_mut(id) {
                if let simple3d_core::scene::Body::Split { plan: was, .. } = &mut node.body {
                    *was = plan;
                }
            }
            id
        } else {
            let parent = self.scene.node(id).parent?;
            let index = self.scene.node(parent).children.iter().position(|&c| c == id).unwrap_or(0);
            // The shape in portable form, taken before it leaves: what Join puts back.
            let original = self.scene.export_subtree(id)?;
            // Removed first so the split can take its name rather than a numbered variant.
            self.scene.remove(id);
            self.scene.add_split(original, plan, parent, index)
        };
        let name = self.scene.node(split).name.clone();
        for (index, piece) in pieces.into_iter().enumerate() {
            // "Box Piece 3", not "Box 3", so pieces do not consume the object numbering series.
            self.scene.add_mesh_named(
                format!("{name} Piece {}", index + 1),
                simple3d_core::mesh_data::MeshData::new(piece),
                split,
                index,
            );
        }
        self.collapsed.remove(&split);
        self.select_only(split);
        Some((name, count))
    }

    // -- a collection's pieces (issue 82) -----------------------------------

    /// Tick or untick a collection piece; `adding` (Ctrl or Shift) adds rather than replaces.
    pub(crate) fn tick_piece(&mut self, id: NodeId, adding: bool) {
        if !adding {
            let only = self.piece_ticks.len() == 1 && self.piece_ticks.contains(&id);
            self.piece_ticks.clear();
            if only {
                return;
            }
            self.piece_ticks.insert(id);
            return;
        }
        if !self.piece_ticks.remove(&id) {
            self.piece_ticks.insert(id);
        }
    }

    /// The object a popup is previewing over, if any (issue 82): the single answer everything that
    /// behaves differently during a preview asks.
    pub(crate) fn preview_subject(&self) -> Option<NodeId> {
        self.split_tool
            .as_ref()
            .map(|tool| tool.target)
            .or_else(|| self.simplify_tool.as_ref().map(|tool| tool.target))
            .or_else(|| self.reassemble_tool.as_ref().map(|tool| tool.target))
            .filter(|&id| self.scene.contains(id))
    }

    /// The collection the properties panel lists: the one selected node, if it is a collection.
    pub(crate) fn listed_collection(&self) -> Option<NodeId> {
        let id = self.primary()?;
        (self.selection.len() == 1 && self.scene.is_collection(id)).then_some(id)
    }
}
