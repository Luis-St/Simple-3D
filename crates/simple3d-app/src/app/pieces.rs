//! The pieces a split produced: holding them, ticking them, and previewing
//! one.

use super::*;
use simple3d_core::scene::NodeId;

impl App {
    /// Bake the selection into stored geometry (issue 80).
    ///
    /// What is captured is what the node evaluates to -- booleans done, pattern
    /// copies laid down -- in the node's *own* frame, so its position, rotation
    /// and scale still mean what they meant and the shape does not move at the
    /// moment it is converted.
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
        // The children went with the old body, so anything selected inside one
        // of them no longer exists.
        self.selection.retain(|id| self.scene.contains(*id));
        if self.selection.is_empty() {
            self.select_only(targets[0]);
        }
        self.status = Status::Info(format!(
            "Converted to {triangles} triangle{} of geometry -- the parameters behind it are gone",
            plural(triangles)
        ));
    }

    /// Stand a split where a node stands, holding `pieces`, and select it
    /// (issue 82). The cutting of a shape into a pattern of cells ends here.
    ///
    /// A node that is *already* a split keeps its identity and the shape it was
    /// made from -- only its pieces are replaced -- which is what makes cutting
    /// a split again a change of pattern rather than a split of a split, and
    /// what keeps one Join back together enough to undo any number of them.
    ///
    /// Returns the name the pieces were made from and how many there are, or
    /// `None` where the document could not be changed at all; the caller has
    /// taken the history snapshot and discards it in that case.
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
            // The shape itself, in the portable form the project file already
            // uses, taken before it leaves the document: it is what the split
            // holds, and what joining the pieces back together puts back.
            let original = self.scene.export_subtree(id)?;
            // Out first, so the split standing in its place can carry its name
            // rather than a numbered variant of it.
            self.scene.remove(id);
            self.scene.add_split(original, plan, parent, index)
        };
        let name = self.scene.node(split).name.clone();
        for (index, piece) in pieces.into_iter().enumerate() {
            // "Box Piece 3", not "Box 3": a piece is not another box, and
            // numbering it as one takes the number out of the series the
            // objects use -- eighty pieces called "Box 1" to "Box 80" leave the
            // next box the user adds to be called "Box 81". The name is taken
            // as it is, since the pieces of one collection are numbered apart
            // by construction.
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

    /// Tick or untick one of a collection's pieces. `adding` is a click with
    /// Ctrl or Shift held, which adds to what is ticked rather than replacing
    /// it.
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

    /// The object an in-place popup is drawing a preview over, if one is
    /// (issue 82).
    ///
    /// One question with one answer, asked by everything that has to behave
    /// differently while a preview is up: the viewport's grid and axes, what is
    /// drawn as the model, the renderables kept ready, and the frame cache's
    /// key. Only the split tool has a preview today; the next tool that grows
    /// one answers here too, and nothing downstream has to learn about it.
    pub(crate) fn preview_subject(&self) -> Option<NodeId> {
        self.split_tool.as_ref().map(|tool| tool.target).filter(|&id| self.scene.contains(id))
    }

    /// The collection whose pieces the properties panel is listing: the one
    /// selected node, when it is a collection at all.
    pub(crate) fn listed_collection(&self) -> Option<NodeId> {
        let id = self.primary()?;
        (self.selection.len() == 1 && self.scene.is_collection(id)).then_some(id)
    }
}
