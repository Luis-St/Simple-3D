//! Undo and redo across every model-mutating action (spec section 7.4).
//!
//! Snapshots rather than inverse operations. A whole `Scene` for a 200-primitive
//! model is a few hundred kilobytes, and taking a copy is the only approach that
//! is *automatically* correct for every edit -- including reparenting, grouping a
//! multi-selection and cutting a subtree, which are exactly the operations an
//! inverse-command scheme gets subtly wrong.
//!
//! Rapid edits to one field coalesce into a single step: the caller passes a
//! coalesce key (`"param:7:width"`), and a second edit with the same key inside
//! the coalesce window reuses the snapshot already taken. A whole drag is one
//! step because the caller records once, before the drag starts.

mod history;
#[cfg(test)]
mod tests;

use crate::scene::Scene;
use std::time::{Duration, Instant};

const DEFAULT_DEPTH: usize = 200;

const COALESCE_WINDOW: Duration = Duration::from_millis(900);

#[derive(Clone)]
struct Snapshot {
    label: String,
    scene: Scene,
}

pub struct History {
    past: Vec<Snapshot>,
    future: Vec<Snapshot>,
    depth: usize,
    open_key: Option<String>,
    open_at: Option<Instant>,
    /// Bumped on every recorded edit; the app compares it against the value it
    /// last saved to decide whether the title bar shows unsaved changes.
    revision: u64,
}

impl Default for History {
    fn default() -> Self {
        History::new()
    }
}
