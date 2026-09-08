//! The per-node renderables the viewport draws hidden bodies from.

use crate::app::App;
use crate::render::Renderable;
use simple3d_core::scene::{NodeId, Visibility};
use std::hash::{Hash, Hasher};

impl App {
    /// The nodes drawn as ghosts: hidden, and asked to be seen anyway. A group
    /// set to ghost carries its children with it, the way hiding it does.
    pub(crate) fn ghosts(&self) -> Vec<NodeId> {
        self.scene
            .depth_first()
            .into_iter()
            .filter(|id| self.scene.node(*id).visibility() == Visibility::Ghost)
            .collect()
    }

    /// A cheap summary of which nodes are ghosts, for the cache key: the
    /// renderables have to be rebuilt when one is turned on or off.
    pub(super) fn ghost_generation(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.ghosts().hash(&mut hasher);
        hasher.finish()
    }

    /// Rebuild the per-node meshes the viewport needs -- the selection outline and
    /// the ghosts -- when either the evaluation or what is selected has changed.
    pub(crate) fn refresh_node_renderables(&mut self) {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.evaluation_generation.hash(&mut hasher);
        self.selection.hash(&mut hasher);
        self.piece_ticks.hash(&mut hasher);
        self.preview_subject().hash(&mut hasher);
        self.ghost_generation().hash(&mut hasher);
        let key = hasher.finish();
        if key == self.renderable_key {
            return;
        }
        self.renderable_key = key;

        let mut fresh = std::collections::BTreeMap::new();
        // Every node the user asked to keep as a ghost, so a subtracted tool
        // body can be seen while it is being positioned (spec section 6.1). A
        // ghost group is drawn as its children, one translucent body each,
        // because that is the assembly the user is placing.
        let mut ghosts: Vec<NodeId> = Vec::new();
        for id in self.ghosts() {
            ghosts.extend(std::iter::once(id).chain(self.scene.descendants(id)));
        }
        ghosts.sort_unstable();
        ghosts.dedup();
        for id in ghosts {
            if let Some(mesh) = self.evaluated.node_meshes.get(&id) {
                fresh.insert(id, Renderable::prepare(mesh));
            }
        }
        // The selection is drawn as *what each selected node evaluates to*, and
        // not one row further down. Descending to the children outlined shapes
        // the result does not contain: a difference's cutter as two rims
        // hanging in mid-air, an intersection's whole uncut box as a cage round
        // the small lens it leaves, a pattern's source child standing where no
        // copy of it does.
        for id in self.top_level_selection() {
            if let Some(mesh) = self.evaluated.result_mesh(id) {
                fresh.insert(id, Renderable::prepare_outlined(&mesh));
            }
        }
        // A ticked piece is outlined the same way, so pointing at one in the
        // viewport is how a piece is found among thousands (issue 82).
        for id in self.piece_ticks.clone() {
            if let Some(mesh) = self.evaluated.result_mesh(id) {
                fresh.insert(id, Renderable::prepare_outlined(&mesh));
            }
        }
        // What a tool is previewing, kept ready whether or not it is selected:
        // "only what is previewed" draws that object *as* the model, and the
        // selection can move on to something else while the tool is open
        // (issue 82).
        if let Some(id) = self.preview_subject() {
            if let std::collections::btree_map::Entry::Vacant(slot) = fresh.entry(id) {
                if let Some(mesh) = self.evaluated.result_mesh(id) {
                    slot.insert(Renderable::prepare_outlined(&mesh));
                }
            }
        }
        self.node_renderables = fresh;
        self.invalidate_image();
    }
}
