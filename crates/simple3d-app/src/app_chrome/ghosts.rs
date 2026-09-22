//! The per-node renderables the viewport draws hidden bodies from.

use crate::app::App;
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
    ///
    /// Asked on every frame, so it reads the nodes in the order the scene
    /// keeps them rather than walking the tree into a list the way [`ghosts`]
    /// does: which nodes are ghosts is all the key needs, not their order.
    ///
    /// [`ghosts`]: App::ghosts
    pub(super) fn ghost_generation(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for id in self.scene.ids().filter(|id| self.scene.node(*id).visibility() == Visibility::Ghost) {
            id.hash(&mut hasher);
        }
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

        let wanted = self.wanted_renderables();
        let cache = self.worker.renderables.clone();
        let mut fresh = std::collections::BTreeMap::new();
        for &(id, outlined) in &wanted {
            // The evaluation thread has usually made these already; what is
            // left -- a node selected since the last evaluation -- is made
            // here, once, and kept for as long as its mesh is.
            if let std::collections::btree_map::Entry::Vacant(slot) = fresh.entry(id) {
                if let Some(renderable) = cache.get(&self.evaluated, (id, outlined)) {
                    slot.insert(renderable);
                }
            }
        }
        cache.retain(&wanted);
        self.node_renderables = fresh;
        self.invalidate_image();
    }

    /// Every single node the viewport draws, with whether it needs the
    /// outline's edge adjacency, in the order it has first claim on a node.
    pub(crate) fn wanted_renderables(&self) -> Vec<crate::render::Wanted> {
        let mut wanted: Vec<crate::render::Wanted> = Vec::new();
        // The selection is drawn as *what each selected node evaluates to*,
        // and not one row further down. Descending to the children outlined
        // shapes the result does not contain: a difference's cutter as two
        // rims hanging in mid-air, an intersection's whole uncut box as a cage
        // round the small lens it leaves, a pattern's source child standing
        // where no copy of it does.
        wanted.extend(self.top_level_selection().into_iter().map(|id| (id, true)));
        // A ticked piece is outlined the same way, so pointing at one in the
        // viewport is how a piece is found among thousands (issue 82).
        wanted.extend(self.piece_ticks.iter().map(|&id| (id, true)));
        // What a tool is previewing, kept ready whether or not it is selected:
        // "only what is previewed" draws that object *as* the model, and the
        // selection can move on to something else while the tool is open
        // (issue 82).
        wanted.extend(self.preview_subject().map(|id| (id, true)));
        // Every node the user asked to keep as a ghost, so a subtracted tool
        // body can be seen while it is being positioned (spec section 6.1). A
        // ghost group is drawn as its children, one translucent body each,
        // because that is the assembly the user is placing. Only a node with a
        // mesh of its own is drawn that way; a group is drawn through them.
        let mut ghosts: Vec<NodeId> = Vec::new();
        for id in self.ghosts() {
            ghosts.extend(std::iter::once(id).chain(self.scene.descendants(id)));
        }
        ghosts.sort_unstable();
        ghosts.dedup();
        for id in ghosts {
            // A node both ghosted and selected keeps the outlined one it was
            // claimed with above, as it always has.
            if self.evaluated.node_meshes.contains_key(&id) && !wanted.iter().any(|&(w, _)| w == id) {
                wanted.push((id, false));
            }
        }
        wanted
    }
}
