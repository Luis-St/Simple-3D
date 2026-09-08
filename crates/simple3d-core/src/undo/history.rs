//! Recording a step, and stepping back and forward through them.

use super::*;
use crate::scene::Scene;
use std::time::Instant;

impl History {
    pub fn new() -> History {
        History {
            past: Vec::new(),
            future: Vec::new(),
            depth: DEFAULT_DEPTH,
            open_key: None,
            open_at: None,
            revision: 0,
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// How many steps are on the undo stack. A gesture that should be one step
    /// can be checked against this rather than against how it feels.
    pub fn undo_len(&self) -> usize {
        self.past.len()
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.past.last().map(|s| s.label.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.future.last().map(|s| s.label.as_str())
    }

    pub fn clear(&mut self) {
        self.past.clear();
        self.future.clear();
        self.open_key = None;
        self.open_at = None;
    }

    /// Call **before** mutating `scene`. `coalesce` groups consecutive edits of
    /// the same thing into one step; pass `None` for an edit that always gets
    /// its own step.
    ///
    /// Returns whether this edit joined the run that was already open, which is
    /// what a caller needs to tell one continuous gesture from a fresh choice.
    pub fn record(&mut self, scene: &Scene, label: &str, coalesce: Option<&str>) -> bool {
        self.revision += 1;
        let coalescing = match (coalesce, &self.open_key, self.open_at) {
            (Some(key), Some(open), Some(at)) => key == open && at.elapsed() < COALESCE_WINDOW,
            _ => false,
        };
        // Refresh the timer either way, so a held-down arrow key keeps extending
        // one step instead of splitting once the first press ages out.
        self.open_key = coalesce.map(|k| k.to_string());
        self.open_at = Some(Instant::now());
        if coalescing {
            return true;
        }
        self.future.clear();
        self.past.push(Snapshot { label: label.to_string(), scene: scene.clone() });
        if self.past.len() > self.depth {
            self.past.remove(0);
        }
        false
    }

    /// Drop the most recent snapshot without restoring it, for an edit that was
    /// recorded and then abandoned -- a drag cancelled with Escape, which puts
    /// the pre-drag values back itself. Keeping the snapshot would leave an undo
    /// step that restores the state it is already in.
    ///
    /// Does *not* touch the redo stack: `record` cleared it when the abandoned
    /// edit opened, and a cancel cannot bring it back.
    pub fn discard_last(&mut self) -> bool {
        self.close();
        self.revision += 1;
        self.past.pop().is_some()
    }

    /// Ends any open coalescing run, so the next edit definitely starts a new
    /// step. Called when the selection changes or a field loses focus.
    pub fn close(&mut self) {
        self.open_key = None;
        self.open_at = None;
    }

    pub fn undo(&mut self, scene: &mut Scene) -> Option<String> {
        let snapshot = self.past.pop()?;
        self.close();
        self.revision += 1;
        self.future.push(Snapshot { label: snapshot.label.clone(), scene: scene.clone() });
        restore(scene, snapshot.scene);
        Some(snapshot.label)
    }

    pub fn redo(&mut self, scene: &mut Scene) -> Option<String> {
        let snapshot = self.future.pop()?;
        self.close();
        self.revision += 1;
        self.past.push(Snapshot { label: snapshot.label.clone(), scene: scene.clone() });
        restore(scene, snapshot.scene);
        Some(snapshot.label)
    }
}

/// Put a snapshot back, **keeping the camera where it is now**.
///
/// A snapshot is the whole `Scene`, camera included, because the camera is
/// saved with the project. Restoring it wholesale would mean undoing a move
/// also threw the view back to wherever it happened to be when the move was
/// made -- which is not what "undo" means to anyone. Undo is over the model;
/// where you are looking from is not part of it.
pub(crate) fn restore(scene: &mut Scene, snapshot: Scene) {
    let camera = scene.camera;
    *scene = snapshot;
    scene.camera = camera;
}
