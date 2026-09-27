//! Undo and redo across every model-mutating action (spec section 7.4).
//!
//! Whole-scene snapshots rather than inverse operations: a few hundred kilobytes for 200
//! primitives, and automatically correct for every edit, including reparenting, grouping and cutting
//! that inverse schemes get subtly wrong. Edits sharing a coalesce key (`"param:7:width"`) within the
//! window reuse one snapshot; a drag records once, before it starts.

mod history;
#[cfg(test)]
mod tests;

use crate::scene::{ComponentId, Scene};
use std::time::{Duration, Instant};

const DEFAULT_DEPTH: usize = 200;

const COALESCE_WINDOW: Duration = Duration::from_millis(900);

#[derive(Clone)]
struct Snapshot {
    label: String,
    scene: Scene,
    /// The components this step's edit made (issue 113), which the application removes on undo since
    /// they are not part of any one scene.
    created: Vec<ComponentId>,
}

pub struct History {
    past: Vec<Snapshot>,
    future: Vec<Snapshot>,
    depth: usize,
    open_key: Option<String>,
    open_at: Option<Instant>,
    /// Bumped per recorded edit; compared with the saved value for the unsaved-changes marker.
    revision: u64,
}

impl Default for History {
    fn default() -> Self {
        History::new()
    }
}
