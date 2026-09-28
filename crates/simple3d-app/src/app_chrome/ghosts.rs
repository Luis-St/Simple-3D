//! The per-node renderables the viewport draws hidden bodies from.

use crate::app::App;
use simple3d_core::scene::{NodeId, Visibility};
use std::hash::{Hash, Hasher};

impl App {
    /// The nodes drawn as ghosts: hidden but asked to be seen; a ghost group carries its children.
    pub(crate) fn ghosts(&self) -> Vec<NodeId> {
        self.scene
            .depth_first()
            .into_iter()
            .filter(|id| self.scene.node(*id).visibility() == Visibility::Ghost)
            .collect()
    }

    /// A cheap summary of which nodes are ghosts, for the cache key. Read in scene order every frame,
    /// since only membership matters, not [`ghosts`]' tree order.
    ///
    /// [`ghosts`]: App::ghosts
    pub(super) fn ghost_generation(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for id in self.scene.ids().filter(|id| self.scene.node(*id).visibility() == Visibility::Ghost) {
            id.hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Rebuild the per-node meshes for the selection outline and ghosts when the evaluation or
    /// selection changed.
    pub(crate) fn refresh_node_renderables(&mut self) {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.evaluation_generation.hash(&mut hasher);
        self.selection.hash(&mut hasher);
        self.piece_ticks.hash(&mut hasher);
        self.preview_subject().hash(&mut hasher);
        self.ghost_generation().hash(&mut hasher);
        self.arrange_tool.as_ref().map(|tool| &tool.targets).hash(&mut hasher);
        let key = hasher.finish();
        if key == self.renderable_key {
            return;
        }
        self.renderable_key = key;

        let wanted = self.wanted_renderables();
        let cache = self.worker.renderables.clone();
        let mut fresh = std::collections::BTreeMap::new();
        for &(id, outlined) in &wanted {
            // Usually made by the evaluation thread already; a newly selected node is made here once.
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

    /// Every node the viewport draws singly, with whether it needs outline adjacency, in claim order.
    pub(crate) fn wanted_renderables(&self) -> Vec<crate::render::Wanted> {
        let mut wanted: Vec<crate::render::Wanted> = Vec::new();
        // Each selected node as what it evaluates to, not its children, which would show cutters and
        // uncut boxes the result does not contain.
        wanted.extend(self.top_level_selection().into_iter().map(|id| (id, true)));
        // Ticked pieces are outlined too, to find them among thousands (issue 82).
        wanted.extend(self.piece_ticks.iter().map(|&id| (id, true)));
        // The previewed object, kept ready since "only what is previewed" draws it as the model (issue 82).
        wanted.extend(self.preview_subject().map(|id| (id, true)));
        // The align tool's objects, drawn again as templates where they would go (issue 70), even once
        // the selection has moved on.
        for &id in self.arrange_tool.iter().flat_map(|tool| &tool.targets) {
            if !wanted.iter().any(|&(w, _)| w == id) {
                wanted.push((id, false));
            }
        }
        // Every ghost node, so a tool body can be positioned (spec section 6.1); a ghost group is drawn
        // as its children with meshes.
        let mut ghosts: Vec<NodeId> = Vec::new();
        for id in self.ghosts() {
            ghosts.extend(std::iter::once(id).chain(self.scene.descendants(id)));
        }
        ghosts.sort_unstable();
        ghosts.dedup();
        for id in ghosts {
            // A node both ghosted and selected keeps its outlined claim from above.
            if self.evaluated.node_meshes.contains_key(&id) && !wanted.iter().any(|&(w, _)| w == id) {
                wanted.push((id, false));
            }
        }
        wanted
    }
}
